# Changelog

All notable changes to Headroom are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

_Nothing yet._

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
