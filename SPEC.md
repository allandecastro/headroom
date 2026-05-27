# SPEC

Technical specification for Headroom v1.

## Goals

1. Show, at a glance from the menu bar, the worst remaining quota across watched AI coding services.
2. Open in a click to a detailed popover with each window's percentage and reset time.
3. Project, via a 7-day burndown, whether the user will hit the weekly cap before reset.
4. Require zero credential paste in the default flow — sign in normally, Headroom handles the rest.

## Non-goals (v1)

- Multi-account support (one Claude account, one GitHub account per Headroom install).
- Team/org rollups (Headroom is a personal tool, not a billing dashboard).
- Cursor / Cody / Codeium integration (see [ROADMAP.md](ROADMAP.md) Phase 4).
- Notifications beyond local desktop alerts (no Slack/Discord webhooks in v1).
- Web sync of usage history.

---

## High-level architecture

```
┌─────────────────────────────────────────────────────────────┐
│ Tauri 2 app                                                 │
│                                                             │
│  ┌──────────────────┐         ┌──────────────────────────┐  │
│  │ React renderer   │ ◄─IPC─► │ Rust backend             │  │
│  │ (popover + UI)   │         │                          │  │
│  └──────────────────┘         │  ┌────────────────────┐  │  │
│                               │  │ orchestrator       │  │  │
│                               │  │  (30s poll loop)   │  │  │
│                               │  └─────────┬──────────┘  │  │
│                               │            │             │  │
│                               │  ┌─────────┴──────────┐  │  │
│                               │  │ sources/           │  │  │
│                               │  │  - claude.rs       │  │  │
│                               │  │  - copilot.rs      │  │  │
│                               │  └─────────┬──────────┘  │  │
│                               │            │             │  │
│                               │  ┌─────────┴──────────┐  │  │
│                               │  │ credentials        │  │  │
│                               │  │  (OS keychain)     │  │  │
│                               │  └────────────────────┘  │  │
│                               └──────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

The renderer never makes network calls itself. The Rust backend owns all credentials, all HTTP traffic, and the polling loop. The renderer subscribes to a `tokens-updated` event and renders whatever the backend emits.

---

## Data sources

### Claude Code

**Endpoint** (undocumented but stable, used by `claude.ai/settings/usage` itself):

```
GET https://claude.ai/api/organizations/{orgId}/usage
Cookie: sessionKey={key}
User-Agent: Mozilla/5.0 ... (must look like a real browser to avoid Cloudflare challenge)
```

The `orgId` comes from a prior call to `GET https://claude.ai/api/organizations`, which returns an array; pick the user's primary org by `uuid`.

**Response shape** (confirmed against a live response — snake_case, no `plan` field):

```json
{
  "five_hour":         { "utilization": 3,  "resets_at": "2026-05-26T17:46:00Z" },
  "seven_day":         { "utilization": 32, "resets_at": "2026-06-02T09:14:00Z" },
  "seven_day_sonnet":  { "utilization": 1,  "resets_at": "2026-06-02T09:14:00Z" },
  "seven_day_opus":    { "utilization": 0,  "resets_at": "2026-06-02T09:14:00Z" },
  "seven_day_omelette":{ "utilization": 0,  "resets_at": "2026-06-02T09:14:00Z" }
}
```

Each window is `{ utilization: f64, resets_at: Option<DateTime> }`. The adapter maps them to UI rows: `five_hour` → "Current session", `seven_day` → "Weekly · 7d", `seven_day_sonnet` → "Sonnet · 7d", `seven_day_opus` → "Opus · 7d", and `seven_day_omelette` → "Claude Design" (hidden unless the user opts in via Settings). There is no plan field, so `plan` is left empty.

**Cloudflare handling.** The endpoint is behind Cloudflare bot mitigation. Generic `curl` user agents get 403s. The Rust HTTP client must send a realistic UA and accept-language header. On 403, the orchestrator marks the source as `unreachable` and surfaces it to the UI rather than retrying in a tight loop.

### GitHub Copilot

**Endpoint** (official, documented):

```
GET https://api.github.com/users/{username}/settings/billing/premium_request/usage
    ?year=2026&month=5
Authorization: Bearer {token}
Accept: application/vnd.github+json
X-GitHub-Api-Version: 2022-11-28
```

The token must be a fine-grained PAT with `Account → Plan → Read-only`, OR a GitHub OAuth token with the equivalent scope (obtained via device flow — see [Auth flows](#auth-flows)).

**Response shape:**

```json
{
  "usageItems": [
    { "product": "Copilot", "grossQuantity": 847.0, "date": "2026-05-01", ... },
    ...
  ]
}
```

Adapter sums `grossQuantity` where `product == "Copilot"`. The monthly limit (50/300/1500) is not exposed by the API and must be set by the user at onboarding, derived from their plan.

**June 1, 2026 migration.** GitHub is replacing premium requests with AI Credits on this date. The same endpoint will return credit amounts in USD rather than request counts. The Copilot source adapter has a `unit` field on its output (`requests` | `credits`) so the UI can render either correctly.

---

## Auth flows

Each source supports a primary "magic" path and a fallback paste path. Both paths produce the same artifact (a Bearer token or `sessionKey`) stored in the OS keychain.

> **Current status (Phase 1).** Onboarding ships **paste-based** for both services — paste the Claude `sessionKey` and paste a Copilot PAT + username + plan. The Claude embedded-webview sign-in below is implemented (`start_claude_signin`) but kept secondary because identity-provider behaviour in the webview is inconsistent. The Copilot **device flow is not yet implemented** (Phase 2).

### Claude — primary: embedded webview

1. User clicks "Sign in with Claude" in onboarding.
2. Headroom opens a Tauri `WebviewWindow` pointing at `https://claude.ai/login`.
3. User authenticates normally (password / Google SSO / magic link).
4. After redirect to `claude.ai`, Headroom reads `sessionKey` via `webview.cookies_for_url("https://claude.ai".parse()?)`.
5. Cookie is written to the keychain under service `headroom`, account `claude.session`.
6. The webview closes.

### Claude — fallback: paste session key

For users who can't run a webview (some Linux distros without webkit2gtk, or air-gapped environments), a hidden "Advanced" panel accepts a manually copied `sessionKey` value. Instructions point at DevTools → Application → Cookies → `claude.ai` → `sessionKey`.

### Copilot — primary: GitHub device flow

1. User clicks "Sign in with GitHub" in onboarding.
2. Headroom requests a device code: `POST https://github.com/login/device/code` with `client_id={CLIENT_ID}` and `scope=read:user`.
3. Response includes `device_code`, `user_code`, `verification_uri`, `interval`.
4. Headroom shows the `user_code` and opens `verification_uri` in the user's default browser.
5. User authorizes the app on GitHub.
6. Headroom polls `POST https://github.com/login/oauth/access_token` at the suggested `interval` until it receives an `access_token`.
7. Token is written to keychain under service `headroom`, account `copilot.token`.

The `CLIENT_ID` is hardcoded in the binary — GitHub device flow does not use a client secret, so this is safe.

### Copilot — fallback: paste a PAT

The "Advanced" panel accepts a fine-grained personal access token. Instructions point at `github.com/settings/tokens?type=beta` with the exact permission scope to enable (`Account → Plan → Read-only`).

---

## Modules

### `sources/`

A trait and per-service implementations. The app state owns a `Vec<Arc<dyn QuotaSource>>` and the orchestrator calls each on every poll tick.

```rust
#[async_trait]
pub trait QuotaSource: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    async fn fetch(&self, creds: &Credentials) -> Result<ServiceStatus, SourceError>;
}

pub struct ServiceStatus {
    pub id: String,
    pub name: String,
    pub state: ServiceState,     // Active | NeedsSetup | AuthRequired | Unreachable
    pub plan: String,
    pub quotas: Vec<Quota>,
    pub error_detail: Option<String>,
}

pub struct Quota {
    pub window: QuotaWindow,     // FiveHour | WeeklyAll | WeeklySonnet | WeeklyOpus | ClaudeDesign | Monthly
    pub label: String,           // human row label, e.g. "Current session"
    pub used: f64,
    pub total: f64,
    pub unit: QuotaUnit,         // Messages | Hours | Requests | UsdCredits | Percent
    pub resets_at: DateTime<Utc>,
    pub advice: Option<String>,  // shown only in the critical state
}
```

A source that has no stored credentials returns `SourceError::MissingCredentials`, which the orchestrator renders as the `NeedsSetup` state (a "Not connected" card) rather than an error.

### `credentials`

Thin wrapper around the `keyring` crate. All values stored under service name `headroom`. Per-account keys:

- `claude.session` — the `sessionKey` cookie value
- `claude.orgId` — cached organization UUID
- `copilot.token` — Bearer token (OAuth or PAT)
- `copilot.username` — GitHub username used in the billing API path
- `copilot.plan` — plan tier ("free" | "pro" | "pro_plus"), used to look up the monthly cap

The renderer writes these via IPC commands (the onboarding flow calls them; the renderer never touches the keychain directly):

| Command | Effect |
| ------- | ------ |
| `set_claude_session(session_key)` | Writes `claude.session` |
| `set_copilot_token(token)` | Writes `copilot.token` |
| `set_copilot_username(username)` | Writes `copilot.username` |
| `set_copilot_plan(plan)` | Validates `plan` against the recognized tiers, then writes `copilot.plan` |
| `clear_credentials(service)` | Deletes every key under the `claude` or `copilot` prefix |

### `commands`

The `#[tauri::command]` IPC handlers exposed to the renderer: credential writes (`set_claude_session`, `set_copilot_*`, `clear_credentials`), settings (`get_settings`, `set_settings`), window control (`open_onboarding`, `open_settings`), autostart (`get_autostart`, `set_autostart`), `refresh_all`, `quit_app`, and `start_claude_signin`.

### `notifications`

Threshold-crossing desktop alerts. `crossed_threshold()` computes the highest enabled threshold a percentage has crossed (critical wins over warning); `notify_thresholds()` fires one notification per crossing and re-arms when the quota drops back below both thresholds.

### `orchestrator`

Background Tokio task. Default tick: 30s, re-read from settings each cycle. On each tick:

1. For each source, call `fetch()` with a 10s timeout.
2. Aggregate results into a snapshot, persisted in memory as `last_snapshot`.
3. Emit `tokens-updated` to the renderer.
4. Update tray state (worst-quota percentage drives the tooltip / title).
5. Run `notify_thresholds()`.

Usage history is recorded here too: each active quota's utilization is sampled into the `history` module (throttled to ~5-minute spacing), persisted to disk, and downsampled into the popover sparkline.

### `tray`

Tray icon state machine. Four image variants (`ok`, `warn`, `crit`, `unreachable`) drive colour from the worst quota. On macOS the worst percentage is shown next to the icon via the native `set_title` API; on Windows it is shown in the hover **tooltip** ("Headroom — N% used"), since Windows has no tray title. A single left-click (matched on button **release**) toggles the popover; the right-click menu offers Open / Set up accounts… / Settings… / Quit. The tray is built **once**, in code (`TrayIconBuilder`) — `tauri.conf.json` must **not** also declare a `trayIcon`, or two icons appear.

---

## Polling strategy

- Default interval: **30 seconds**, configurable in Settings between 15s and 5min.
- On window open: immediate refresh, then resume normal cadence.
- On network error: exponential backoff (30s → 1min → 2min → 5min, cap), with the affected source marked `unreachable` after three consecutive failures.
- On auth error (401/403 not from Cloudflare): mark source as `auth_required`, surface a "Sign in again" button in its card.
- Cloudflare 403 detection: response body contains `<title>Just a moment...</title>` or similar. Mark `unreachable` with reason `cloudflare_challenge`, retry on user-initiated refresh only.

---

## State machine — service status

```
            ┌─────────────┐
            │ needs_setup │  (no stored credentials)
            └──────┬──────┘
                   │ user signs in / pastes credentials
                   ▼
            ┌──────────┐
   ┌────────┤  active  ├─────────┐
   │        └─────┬────┘         │
   │              │              │
   │ 401/403      │ Cloudflare   │ network fail
   │              │              │ / timeout
   ▼              ▼              ▼
┌────────────┐ ┌──────────────┐ ┌─────────────┐
│ auth_      │ │ unreachable  │ │ unreachable │
│ required   │ │ (challenge)  │ │ (network)   │
└────────────┘ └──────────────┘ └─────────────┘
```

The renderer displays the current state per service (Rust enum `ServiceState`: `Active | NeedsSetup | AuthRequired | Unreachable`). `active` shows quotas; `needs_setup` shows a "Not connected — open Set up accounts…" card; `unreachable` shows the error detail; `auth_required` shows "Sign in again."

---

## Notifications

Two configurable thresholds drive local desktop notifications: a **warning** (orange) and a **critical** (red), each a percentage where `0` means off and critical takes precedence. On every poll, each active quota's percentage is checked via `crossed_threshold()`; the first time it crosses an enabled threshold, one notification fires ("Heads up" / "Critical", with the service, quota label, and percentage). The crossing is recorded per `service:quota` so it does not re-fire, and re-arms once the quota drops back below both thresholds.

> On Windows, toast notifications are attributed to the process's registered AppUserModelID. A correctly **installed** build (MSI/NSIS, which creates a Start Menu shortcut) shows "Headroom"; a loose `.exe` launched from a shell inherits that shell's identity instead.

---

## Storage

| What                | Where                                      | Format    | Status      |
| ------------------- | ------------------------------------------ | --------- | ----------- |
| Credentials         | OS keychain (service `headroom`)           | string    | implemented |
| User preferences    | `dirs::config_dir()/headroom/settings.json`| JSON      | implemented |
| Cached snapshots    | in-memory only (`last_snapshot`)           | n/a       | implemented |
| Usage history       | `dirs::data_dir()/headroom/history.jsonl`  | JSONL     | implemented |

**Settings** (`settings.json`, written atomically and clamped on load): `poll_interval_secs`, `theme` (`auto`/`light`/`dark`), `show_tray_percentage`, `notify_warn_pct` (orange, 0 = off), `notify_crit_pct` (red, 0 = off), `show_claude_design`. Launch-at-login is managed by `tauri-plugin-autostart`, not stored here. Saving settings emits `settings-updated` so open windows react live (theme, Claude Design toggle).

The history file is append-only JSONL, one line per `{ ts, service, window, used_pct }`, loaded on startup and pruned to a 30-day retention horizon.

---

## Tray icon — visual states

See [DESIGN_SYSTEM.md § Tray icons](DESIGN_SYSTEM.md#tray-icons). Four PNG assets at @1x and @2x:

- `tray-ok.png` — green fill, healthy
- `tray-warn.png` — amber fill, 80–94%
- `tray-crit.png` — red fill, ≥95%
- `tray-unreachable.png` — dashed gray, no data

---

## Edge cases worth listing explicitly

- **User has both Claude Code OAuth credentials and a claude.ai session in browser**: prefer Claude Code OAuth in keychain (more stable, no Cloudflare). Fall back to webview/cookie only if it fails.
- **Daylight saving transition**: all reset times are stored as UTC; the renderer formats them in the user's local TZ at display time. Burndown chart x-axis is in local time.
- **User changes plan mid-month**: the `copilot.plan` keychain entry is editable from Settings. Re-detection from a credentials hint (e.g. user signs in fresh) is automatic.
- **Anthropic ships an official usage API**: the `claude.rs` source has a clean interface; adding a second strategy (Bearer vs Cookie) is a 30-line change.
- **Multiple monitors with different DPI**: tray icon must render correctly at 16, 22, 32 px logical. SVG source rasterized at build time into all sizes.
