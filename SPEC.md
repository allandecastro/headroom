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
- Cursor / Codex / Gemini / Perplexity integration (planned — open an issue if you'd build the adapter).
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
  "five_hour": { "utilization": 3, "resets_at": "2026-05-26T17:46:00Z" },
  "seven_day": { "utilization": 32, "resets_at": "2026-06-02T09:14:00Z" },
  "seven_day_sonnet": { "utilization": 1, "resets_at": "2026-06-02T09:14:00Z" },
  "seven_day_opus": { "utilization": 0, "resets_at": "2026-06-02T09:14:00Z" },
  "seven_day_omelette": { "utilization": 0, "resets_at": "2026-06-02T09:14:00Z" }
}
```

Each window is `{ utilization: f64, resets_at: Option<DateTime> }`. The adapter maps them to UI rows: `five_hour` → "Current session", `seven_day` → "Weekly · 7d", `seven_day_sonnet` → "Sonnet · 7d", `seven_day_opus` → "Opus · 7d", and `seven_day_omelette` → "Claude Design" (hidden unless the user opts in via Settings). There is no plan field, so `plan` is left empty.

**Cloudflare handling.** The endpoint is behind Cloudflare bot mitigation. Generic `curl` user agents get 403s. The Rust HTTP client must send a realistic UA and accept-language header. On 403, the orchestrator marks the source as `unreachable` and surfaces it to the UI rather than retrying in a tight loop.

### GitHub Copilot

**Endpoint** (internal):

```
GET https://api.github.com/copilot_internal/user
Authorization: Bearer {token}
Accept: application/json
```

The token is any GitHub token — a classic/OAuth token or PAT, no billing-specific permission. See [Auth flows](#auth-flows). This is the same endpoint the GitHub Copilot editor extensions use; it is undocumented and may change, but it returns the plan, per-feature quotas, and reset date in one call, so caps are read from the response instead of a hardcoded table.

**Response shape** (confirmed against a live response):

```json
{
  "copilot_plan": "individual",
  "token_based_billing": true,
  "quota_reset_date_utc": "2026-06-30T22:00:00.000Z",
  "quota_snapshots": {
    "premium_interactions": { "entitlement": 300, "quota_remaining": 120, "unlimited": false },
    "chat":                 { "entitlement": 200, "quota_remaining": 184, "unlimited": false },
    "completions":          { "entitlement": 2000, "quota_remaining": 2000, "unlimited": false }
  }
}
```

Under token-based billing the `premium_interactions` quota is the **AI Credits** allowance. The adapter surfaces a single headline quota, picking the first _bounded_ entry (`unlimited == false` and `entitlement > 0`) in priority order `premium_interactions → chat → completions → any other`. This means a Pro/Pro+ account shows AI Credits, while a free/individual account (which reports `premium_interactions` with `entitlement: 0`) falls through to its `chat` allowance. For the chosen quota: `used = entitlement − quota_remaining`, `total = entitlement`, reset parsed from `quota_reset_date_utc` (falling back to the start of next month). Plans where every quota is unlimited surface no budget row.

---

## Auth flows

Each source supports a primary "magic" path and a fallback paste path. Both paths produce the same artifact (a Bearer token or `sessionKey`) stored in the OS keychain.

> **Current status.** Claude has a magic webview sign-in (`start_claude_signin`) with a paste-session-key fallback under "Advanced". Copilot has a **"Sign in with GitHub" OAuth device flow** (`start_copilot_signin`) with a paste-token fallback under "Advanced". Both work because `copilot_internal/user` accepts a plain GitHub user token (the old billing endpoint required a fine-grained PAT with `Account → Plan: Read-only` that no OAuth scope could grant).

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
2. `start_copilot_signin` POSTs to `github.com/login/device/code` with the public OAuth App `client_id` (device flow needs no secret, so the id ships in the app and is shared by every install) and scope `read:user`.
3. Headroom shows the returned `user_code` (with a copy button) and opens `verification_uri`; the onboarding window is pinned on top so it isn't lost behind the browser.
4. The backend polls `github.com/login/oauth/access_token` (`grant_type=…device_code`), honoring `authorization_pending` / `slow_down`, until GitHub mints a user token.
5. The token is written to the keychain under `copilot.token` and `copilot-signed-in` is emitted (or `copilot-signin-error` with a message). The OAuth App must have **device flow enabled**.

### Copilot — fallback: paste a GitHub token

Under "Advanced", the user can paste any GitHub personal access token (classic or fine-grained) — no specific permission is required — stored under `copilot.token`. The form links to `github.com/settings/tokens/new`. Either way, the plan tier and quota caps come from `copilot_internal/user`, so the user never supplies a username or plan.

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
- `copilot.token` — GitHub token (any classic/OAuth token or PAT)

`copilot.username` / `copilot.plan` are legacy keys from the old billing API. They are no longer written, but `clear_credentials` still deletes them so sign-out cleans up upgraded installs.

The renderer writes these via IPC commands (the onboarding flow calls them; the renderer never touches the keychain directly):

| Command                           | Effect                                                                    |
| --------------------------------- | ------------------------------------------------------------------------- |
| `set_claude_session(session_key)` | Writes `claude.session`                                                   |
| `set_copilot_token(token)`        | Writes `copilot.token`                                                    |
| `clear_credentials(service)`      | Deletes every key under the `claude` or `copilot` prefix                  |

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

Usage history is recorded here too: each active quota's utilization is sampled into the `history` module (throttled to ~5-minute spacing), persisted to disk, and downsampled into the popover sparkline. From that series the `projection` module derives the **recent-burn-rate pace** for the long (weekly/monthly) windows: `recent_delta` returns Δutilization over the last 24h measured against **wall-clock** (idle time included — the honest basis for a calendar-reset quota, since the reset fires regardless of activity). If the in-window samples straddle a stretch longer than an hour — the app was closed — the rate is being averaged across time we never observed, so the result carries a `low_confidence` flag; the UI shows it muted as _"rough (history gap)"_ and holds back the over-pace warning rather than re-anchoring the lookback (anchoring to the post-reopen burst would over-state the daily rate whenever the gap was just idle sleep).

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

| What             | Where                                       | Format | Status      |
| ---------------- | ------------------------------------------- | ------ | ----------- |
| Credentials      | OS keychain (service `headroom`)            | string | implemented |
| User preferences | `dirs::config_dir()/headroom/settings.json` | JSON   | implemented |
| Cached snapshots | in-memory only (`last_snapshot`)            | n/a    | implemented |
| Usage history    | `dirs::data_dir()/headroom/history.jsonl`   | JSONL  | implemented |

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
- **User changes plan mid-month**: nothing to do — the plan, entitlement, and reset date come from `copilot_internal/user` on every poll, so a tier change is picked up automatically.
- **Anthropic ships an official usage API**: the `claude.rs` source has a clean interface; adding a second strategy (Bearer vs Cookie) is a 30-line change.
- **Multiple monitors with different DPI**: tray icon must render correctly at 16, 22, 32 px logical. SVG source rasterized at build time into all sizes.
- **App closed for a stretch, then reopened**: the live percentages snap straight back to the server-enforced numbers on the next poll, so nothing is lost there. Only the recent-burn-rate pace is affected — its lookback now spans a gap — so it's flagged `low_confidence` and shown as _"rough (history gap)"_ until the sampled history is continuous again (see `orchestrator` / `projection`).
