# Changelog

All notable changes to Headroom are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Multiple GitHub Copilot accounts.** Connect several GitHub accounts (e.g. a
  personal seat and a work seat, or seats in different orgs) and Headroom shows
  one card per account, each with its own quota and history. Manage them under
  Settings → _GitHub Copilot accounts_: add, remove, and rename each account.
  Because Copilot's per-user endpoint exposes no org, each card is labelled with
  its GitHub login by default and is renameable (e.g. to the org name). An
  existing single sign-in migrates automatically on first launch.

### Changed

- **The popover hides services you haven't connected.** Only signed-in services
  appear; when nothing is connected, a small "No accounts connected" prompt links
  to setup. Settings still lists everything to connect or disconnect.

## [1.4.0] — 2026-06-03

### Added

- **One-click updates.** "Check for updates" no longer just opens a browser —
  clicking **Update now** (popover banner) or **Update to vX** (Settings)
  downloads the new release, installs it, shows download progress, and relaunches
  into the new version. Built on `tauri-plugin-updater` with minisign-signed
  artifacts and a published `latest.json` manifest. ([#42])

### Notes

- Self-install runs on **Windows** (MSI — shows a UAC prompt) and **Linux
  AppImage**. **macOS** (notarized code signing still deferred) and **Linux
  `.deb`** keep the previous behavior: the button opens the release page for a
  manual download.
- Auto-install works only **from this release onward** — earlier builds don't
  carry the updater, so they update to 1.4.0 via the browser one last time.

[#42]: https://github.com/allandecastro/headroom/pull/42

## [1.3.3] — 2026-06-02

### Fixed

- **Settings window no longer overlaps the taskbar.** The auto-fit logic sized
  the window to the full content height with no clamp, so as the page grew it
  slipped behind the taskbar. Windows are now clamped to the monitor work area,
  and the Settings/Onboarding panels scroll when content exceeds it. ([#40])

### Changed

- **Theme-aware scrollbar** — the scrollbar (exposed by the fix above) is now a
  thin, subtle pill that matches light/dark instead of the blank OS gutter. ([#40])

### Added

- **Claude diagnostics** — Settings → About now has a single _Diagnostics_ row
  with **Claude** and **Copilot** copy buttons, each dumping that service's raw
  usage payload (credential redacted) to the clipboard for bug reports. Claude
  gains the same escape hatch Copilot had. ([#40])

[#40]: https://github.com/allandecastro/headroom/pull/40

## [1.3.2] — 2026-06-02

### Fixed

- **Migrated individual seats now classify as AI Credits, not request counts.**
  Under `token_based_billing`, **any** capped headline quota is treated as the
  AI-Credits regime (`AiCreditsCapped`) — covering an individual plan's personal
  credits, a user-level budget, or a Free plan's chat cap — while a legacy
  (non-token-based) seat stays `PremiumRequests`. Previously only
  `premium_interactions` was routed to credits, so a migrated Free seat showed as
  a plain request count. The row label stays honest (e.g. a Free chat cap shows
  "Chat", not "AI Credits"). ([#37])

### Changed

- **Clear copy for every Copilot quota state — nothing renders blank.** ([#38])
  - **Capped** rows show the bar + percent with a `"{remaining} / {entitlement}
    left · resets …"` subline (raw numbers only — no hardcoded unit word, since
    the unit varies by quota and plan).
  - **Pooled** org seats read _"Pooled — no individual quota"_ with a one-line
    explanation and an admin-budget hint in the tooltip.
  - **Unrecognized** payloads read _"Couldn't read your quota format"_ and keep
    the "Copy raw payload" action.
  - **Failure states** each get their own line instead of one generic message:
    expired/invalid token → _"re-authenticate"_, offline/timeout → _"check your
    connection"_, rate-limited (429) → _"try again shortly"_, other errors →
    _"GitHub returned an error (status)"_. Loading reads _"Checking usage…"_.

[#37]: https://github.com/allandecastro/headroom/pull/37
[#38]: https://github.com/allandecastro/headroom/pull/38

## [1.3.1] — 2026-06-02

### Fixed

- **Copilot card no longer renders blank after the AI-Credits migration.** GitHub
  moved all Copilot plans from premium requests to usage-based AI Credits on
  2026-06-01, which reshaped the (undocumented) `copilot_internal/user` payload.
  Parsing is now **regime-aware**, driven by fields confirmed in three real
  payloads (Business, Free, legacy Pro+) — never guessed. It surfaces the first
  **capped** quota (`unlimited:false && entitlement>0`) in priority order
  `premium_interactions → chat → completions`, normalizing into: **AI Credits —
  capped** (`premium_interactions` under `token_based_billing` — an individual
  plan's personal credits or a user budget), **premium requests** (a bounded
  request count — legacy grandfathered, or a migrated Free plan's real chat /
  completions caps), **AI Credits — pooled** (a token-based org/Business seat
  whose metered quota is `has_quota` but `unlimited`: credits live at the org
  level, not exposed per-user → _"org-managed (pooled) — no individual quota"_
  with **no bar and no count**, since its `percent_remaining: 100` / `remaining:
  0` would otherwise mislead), or **unknown** (nothing usable → _"couldn't read
  usage"_ + **Copy diagnostics**). `token_based_billing` marks *migration*, not
  "credits" — it's `true` even on Free, whose chat/completions remain real
  request caps. No plan assumptions, no hardcoded caps. Reset-date parsing uses a
  defensive fallback chain (`quota_reset_date_utc` → `quota_reset_date` → start of
  next month).

### Added

- **Copilot diagnostics** — Settings → About → _Copy_ grabs the raw (token-redacted)
  `copilot_internal/user` payload to the clipboard, so the real post-migration
  shape can be reported. Same action appears on the "couldn't read usage" state.

## [1.3.0] — 2026-06-02

### Added

- **Update notifications.** Headroom checks GitHub for a newer release (on
  launch, then every ~6 hours) and, when one is out, shows a one-time desktop
  notification plus a dismissible _"Headroom vX.Y.Z available · Download"_ banner
  in the popover that opens the release page. Settings gains a state-aware
  _"Check for updates"_ button (on-demand: Checking… → Up to date ✓ / Download
  vX →) and a _"Check for updates automatically"_ toggle. It's **notify-only** —
  it links you to the download, it doesn't auto-install (in-app auto-update needs
  code signing). Uses the public Releases API; no token, nothing leaves your
  machine beyond the version check. ([#32])

[#32]: https://github.com/allandecastro/headroom/pull/32

## [1.2.1] — 2026-06-02

### Fixed

- **Recent-burn-rate pace no longer misleads after the app has been closed.**
  The pace is computed from locally-sampled history, which has holes whenever
  Headroom wasn't running (sleep, quit). The live quota percentages are always
  the server-enforced numbers, but a pace whose lookback straddled an
  app-closed gap was being averaged across time it never observed — and could
  trip the red "over pace" warning off it. The slope now stays an honest
  wall-clock rate (the right basis for a calendar-reset quota) and, when its
  lookback spans a gap, renders muted as _"Burning ~N%/day · rough (history
  gap)"_ with the over-pace alarm suppressed until the history is continuous
  again. ([#30])

[#30]: https://github.com/allandecastro/headroom/pull/30

## [1.2.0] — 2026-06-01

### Added

- **"Sign in with GitHub" for Copilot** — a one-click OAuth device-flow sign-in:
  Headroom shows a short code (with a copy button), opens
  `github.com/login/device`, and stores the token automatically once you
  authorize. The manual token paste stays available under "Advanced". Now
  possible because `copilot_internal/user` accepts a plain user token (the old
  billing endpoint needed a fine-grained PAT no OAuth scope could grant).

### Fixed

- The onboarding window no longer slips behind the browser during GitHub
  sign-in — it's pinned on top while the device code is shown, so you can read
  and copy it.
- The Settings "About" version now reflects the actual app version instead of a
  hardcoded `v1.0.0`.

## [1.1.0] — 2026-06-01

### Changed

- **Copilot now reads from `copilot_internal/user`** instead of the per-user
  billing endpoint. The plan, quota cap, and reset date come straight from the
  response — supporting GitHub's token-based **AI Credits** model — so caps are
  no longer hardcoded per tier. The headline quota is AI Credits
  (`premium_interactions`) when present, falling back to the account's `chat`
  allowance on free/individual plans.
- **Copilot onboarding is now a single token paste.** Any GitHub token works —
  the fine-grained `Account → Plan: Read-only` PAT is no longer required, and
  the username and plan fields are gone (both are derived from the API). Tokens
  created under the old flow keep working. The Settings plan picker was removed;
  the plan is shown read-only from your account. Legacy `copilot.username` /
  `copilot.plan` keychain entries are cleared on sign-out.

## [1.0.2] — 2026-06-01

### Fixed

- **Relaunching while signed out** surfaced nothing usable — the app showed the
  empty popover instead of onboarding, so reconnecting meant opening the tray
  menu and clicking "Set up accounts…". A relaunch now routes to onboarding
  when no account is connected. ([#23], [#24])
- **Signing in with a different Claude account** was impossible. Sign-out only
  deleted the keychain entry, leaving the embedded webview's session cookie to
  silently re-authenticate the previous account on the next sign-in. Sign-out
  now clears the webview's browsing data so the next sign-in starts fresh.
  ([#23], [#24])

[#23]: https://github.com/allandecastro/headroom/issues/23
[#24]: https://github.com/allandecastro/headroom/pull/24

## [1.0.1] — 2026-05-30

### Fixed

- **Launch at startup** silently failing after a Windows reboot when the
  registry slot pointed at a stale dev-build exe path. Release builds now
  refresh the registered path on launch (self-heal), and debug builds refuse
  to write a `target\debug\headroom.exe` path into the registry. The Settings
  toggle reverts visually when the backend rejects. ([#20], [#21])

[#20]: https://github.com/allandecastro/headroom/issues/20
[#21]: https://github.com/allandecastro/headroom/pull/21

## [1.0.0] — 2026-05-29

First **public** release. Headroom ships a complete picture of your AI coding
quotas — Claude (current session, weekly all-models, weekly Sonnet, weekly Opus,
optional Claude Design) and GitHub Copilot (monthly premium requests) — in the
system tray, with the burndown projection, sparkline, recent-burn-rate pace,
threshold notifications, single-instance lock, and autostart all live.

> Renamed from the planned 0.1.0 to 1.0.0 for the first public-share tag — the
> feature set is well past an MVP.

### Added

- **Tray app** built with Tauri 2 (Rust backend, React renderer). Single
  left-click toggles the popover; right-click opens Open / Set up accounts /
  Settings / Quit; the icon colour-codes from the worst quota across services.
- **Live quota sources** for Claude Code (`/api/.../usage`) and GitHub Copilot
  (`/users/{u}/settings/billing/premium_request/usage`), each with a 10-second
  fetch budget and a state machine of `Active | NeedsSetup | AuthRequired |
Unreachable`.
- **"Sign in with Claude"** via an embedded webview that captures the
  `sessionKey` cookie.
- **Copilot paste-a-PAT onboarding** with a built-in _"Create one on GitHub →"_
  button that opens the fine-grained PAT page deep-linked to the right scope
  (the billing endpoint requires a permission classic OAuth scopes can't grant
  — see SPEC.md and FAQ.md).
- **Paste fallback** for Claude (session key).
- **7-day burndown projection** + popover sparkline. Quota rows are
  click-to-expand: collapsed shows the always-visible _"On track · ~N% by
  reset"_ line (or _"On track to exceed · full in Xd"_ when pacing past the
  cap); expanded shows a larger trend chart with min/now/max and the
  **recent-burn-rate pace** — _"Burning N%/day · M%/day keeps you on track"_,
  derived from the last 24h of history samples. Usage history is recorded every
  ~5 minutes, persisted to disk as JSONL, and pruned at 30 days.
- **Threshold notifications** — configurable orange (heads-up) and red
  (critical) percentage thresholds, fired once per crossing and re-armed on
  drop-back. Critical is forced strictly above heads-up.
- **Settings window** — refresh interval, theme (auto / light / dark), tray
  percentage toggle, launch at login, optional Claude Design metric,
  notification sliders, per-service Re-auth / Sign-out, GitHub + LinkedIn
  buttons, and a _Check for updates_ link to the Releases page.
- **Single-instance lock** — a second launch surfaces the running tray
  instead of starting a new process.
- **Launch at login** via `tauri-plugin-autostart`.
- **Credentials in the OS keychain** (Windows Credential Manager / macOS
  Keychain / Linux Secret Service) under service `headroom`.
- **Atomic settings persistence** at `dirs::config_dir()/headroom/settings.json`
  with sanitization on load.
- **41 backend tests** covering source classification, threshold crossing /
  re-arm, projection, history downsample / throttle / prune, settings
  clamping, credentials key sets, and source wiremock contracts.

### Fixed

- Duplicate tray icon caused by `trayIcon` being declared both in
  `tauri.conf.json` and code (the config entry was removed).
- Tray click on a minimized popover now restores instead of hiding (a minimized
  window still reports `is_visible`).
- Settings / onboarding windows re-centre within the usable work area, so the
  bottom never slips behind the taskbar.
- Opaque window background — no white flash on overscroll.
- A rejected credential now surfaces as `AuthRequired` ("Sign in again")
  instead of being lumped into `Unreachable`.

### Notes

- macOS and Linux builds are produced by the release pipeline but most
  development and live testing happens on Windows; please file an issue if
  anything platform-specific is off.
- Notifications attribute to "Headroom" only when launched from the
  **installed** build — registers an AppUserModelID on Windows. A loose `.exe`
  launched from a shell will show the shell's name instead.
