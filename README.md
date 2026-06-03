<p align="center">
  <img src="assets/headroom-app-icon.svg" alt="Headroom" width="120" height="120" />
</p>

<h1 align="center">Headroom</h1>

<p align="center">
  <strong>Know your headroom — a menu bar app that tracks Claude Code and GitHub Copilot quotas before you hit them.</strong>
</p>

<p align="center">
  <a href="https://github.com/allandecastro/headroom/releases/latest"><img src="https://img.shields.io/github/v/release/allandecastro/headroom?style=for-the-badge&color=blue&label=release" alt="Latest release" /></a>
  <img src="https://img.shields.io/badge/license-MIT-green?style=for-the-badge" alt="License" />
  <a href="https://github.com/allandecastro/headroom/actions/workflows/ci.yml?query=branch%3Amain"><img src="https://github.com/allandecastro/headroom/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI" /></a>
  <a href="https://github.com/allandecastro/headroom/actions/workflows/release.yml"><img src="https://github.com/allandecastro/headroom/actions/workflows/release.yml/badge.svg" alt="CD" /></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/macOS-000000?style=for-the-badge&logo=apple&logoColor=white" alt="macOS" />
  <img src="https://img.shields.io/badge/Windows-0078D6?style=for-the-badge&logo=windows&logoColor=white" alt="Windows" />
  <img src="https://img.shields.io/badge/Linux-FCC624?style=for-the-badge&logo=linux&logoColor=black" alt="Linux" />
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Tauri_2-24C8DB?style=for-the-badge&logo=tauri&logoColor=white" alt="Tauri 2" />
  <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/React-61DAFB?style=for-the-badge&logo=react&logoColor=black" alt="React" />
  <img src="https://img.shields.io/badge/TypeScript-3178C6?style=for-the-badge&logo=typescript&logoColor=white" alt="TypeScript" />
</p>

<p align="center">
  <a href="#features">Features</a> &bull;
  <a href="#install">Install</a> &bull;
  <a href="#development">Development</a> &bull;
  <a href="#documentation">Documentation</a>
</p>

---

Headroom sits in your menu bar and shows, at a glance, how much of your AI coding assistant budget you have left — across the rolling 5-hour window, the 7-day weekly cap, and the monthly Copilot allowance. It tells you when to switch from Opus to Sonnet, how long until the next reset, and projects whether you'll make it to Monday at your current pace.

It is built for developers on Claude Pro/Max and Copilot Pro/Pro+ who actually use these tools all day and have run into the "usage limit reached" wall mid-task.

---

## Screenshots

<p align="center">
  <img src="docs/screenshots/widget.png" alt="Headroom popover showing live Claude and multiple GitHub Copilot account quotas" width="320" />
</p>

<p align="center"><sub>The menu-bar popover — live quotas for Claude and every connected GitHub Copilot account, with reset countdowns and burndown projection.</sub></p>

<p align="center">
  <img src="docs/screenshots/setup.png" alt="Onboarding window" width="360" />
  &nbsp;
  <img src="docs/screenshots/settings.png" alt="Settings window with multiple GitHub Copilot accounts" width="360" />
</p>

<p align="center"><sub>Onboarding &nbsp;·&nbsp; Settings — connect Claude and manage multiple GitHub Copilot accounts (add, rename, remove).</sub></p>

<p align="center">
  <img src="docs/screenshots/copilot-business-quota.png" alt="GitHub Copilot Business showing a per-user AI-Credits quota" width="360" />
  &nbsp;
  <img src="docs/screenshots/copilot-business-pooled.png" alt="GitHub Copilot Business with org-pooled credits and no per-user quota" width="360" />
</p>

<p align="center"><sub>GitHub Copilot Business — a per-user AI-Credits quota when one is assigned &nbsp;·&nbsp; org-pooled credits, where there's no per-user usage to show.</sub></p>

---

## Features

- **Live quota tracking** for Claude (current session, weekly all-models, weekly Sonnet, weekly Opus, optional Claude Design) and GitHub Copilot (monthly AI Credits / premium requests, with the plan and cap read live from your account). The percentages are the **server-enforced** numbers — read straight off the same endpoints `claude.ai/settings/usage` and your GitHub account use — never reconstructed by parsing local logs, so they can't drift from the quota that's actually enforced.
- **Tray icon** colour-coded green / amber / red from the worst quota across services; hover shows the percentage; the tray menu offers Open / Set up accounts / Settings / Quit.
- **7-day burndown** — a per-quota sparkline of recent utilization plus an _"On track · ~N% by reset"_ projection that flips to _"On track to exceed · full in Xd"_ if you're pacing past the cap. The recent-burn-rate pace is the one figure derived from your locally-sampled history rather than the server; if that history has a gap (the app was closed for a stretch) the pace is shown muted as _"rough (history gap)"_ rather than raising a false alarm.
- **Magic sign-in for both** — _"Sign in with Claude"_ opens an embedded webview that grabs the session cookie; **Copilot** has _"Sign in with GitHub"_ (OAuth device flow — enter a short code, no token to create). Each has a paste fallback under _Advanced_ (a Claude session key, or any GitHub token). The plan and quota cap are read from your account (see [SPEC.md § Auth flows](SPEC.md#auth-flows)).
- **Multiple GitHub Copilot accounts** — connect more than one GitHub account (a personal seat and a work seat, or seats in different orgs) and Headroom shows one card per account, each with its own quota and history. Add, remove, and rename accounts under Settings → _GitHub Copilot accounts_ (the per-user API has no org name, so each is labelled by its login and renameable to the org).
- **Credentials in the OS keychain** — Windows Credential Manager / macOS Keychain / Secret Service on Linux. Nothing leaves your machine.
- **Configurable threshold notifications** — orange "heads-up" and red "critical" alerts at user-set percentages, fired once per crossing.
- **One-click updates** — checks GitHub for newer releases (on launch + every ~6h) and surfaces a one-time desktop alert plus an **Update now** banner in the popover; clicking it downloads, installs, and relaunches the new version (Windows MSI + Linux AppImage; macOS and `.deb` open the release page). Check on demand or toggle it off in Settings.
- **Launch at login** + **single-instance lock** — second launches surface the running tray instead of stacking icons.
- **Live theme switching** (auto / light / dark) and a popover that auto-fits its content above the taskbar.

---

## Status

**Stable.** Both Claude and Copilot run live against the official endpoints, with credentials in the OS keychain; the burndown + sparkline + recent-burn-rate pace, threshold notifications, autostart, single-instance lock, and the click-to-expand chart all ship in the popover. Recent releases added **multiple GitHub Copilot accounts** (one card per account), **one-click updates** (download-install-relaunch from the popover), **"Sign in with GitHub"** for Copilot, regime-aware **GitHub AI-Credits** handling (after GitHub's 2026 billing migration), and per-service **diagnostics**. The latest version is on the [Releases](https://github.com/allandecastro/headroom/releases) page; see [CHANGELOG.md](CHANGELOG.md) for full notes and [FAQ.md § Scope](FAQ.md#scope) for what's next.

---

## Install

### From a release

Grab the installer for your platform from the [Releases](https://github.com/allandecastro/headroom/releases) page:

- **Windows:** `Headroom_x.y.z_x64_en-US.msi` (per-user install; registers the AppUserModelID so toast notifications correctly attribute to "Headroom").
- **macOS:** `Headroom_x.y.z_aarch64.dmg` / `_x64.dmg`.
- **Linux:** `Headroom_x.y.z_amd64.AppImage` or `.deb`.

After installing, launch from the Start Menu / Launchpad and look in your system tray (Windows 11 may hide new tray icons under the `^` overflow — drag the icon out once and it stays visible).

### From source

You need Rust (1.78+) and Node.js (20+) installed.

```bash
git clone https://github.com/allandecastro/headroom.git
cd headroom
npm install
npm run tauri dev
```

The first launch will be slow — Cargo compiles every Rust dependency on the cold path. Subsequent launches take a few seconds.

---

## Development

### Project layout

```
headroom/
├── src/                    # React popover + settings UI
│   ├── components/         # TokenCard, onboarding, shared UI primitives
│   ├── lib/                # Tauri IPC wrappers, formatters, useFitWindow
│   ├── App.tsx             # Popover
│   ├── SettingsPanel.tsx   # Settings window
│   ├── OnboardingFlow.tsx  # Onboarding window
│   └── main.tsx
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── sources/        # QuotaSource trait + claude.rs, copilot.rs
│   │   ├── commands.rs     # Tauri IPC handlers exposed to the renderer
│   │   ├── credentials.rs  # OS keychain wrapper
│   │   ├── history.rs      # Persisted per-quota usage time series
│   │   ├── notifications.rs# Threshold-crossing desktop alerts
│   │   ├── orchestrator.rs # Poll loop + classification
│   │   ├── projection.rs   # Burndown projection + recent-burn-rate pace
│   │   ├── settings.rs     # User preferences, atomic JSON persist
│   │   ├── tray.rs         # Tray icon state machine
│   │   ├── lib.rs          # AppState + run() wiring
│   │   └── main.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── docs/                   # Mockups + screenshots used in this README
└── .github/                # Issue / PR templates + CI + release workflows
```

### Useful commands

| Command                                                                    | What it does                                          |
| -------------------------------------------------------------------------- | ----------------------------------------------------- |
| `npm run dev`                                                              | Vite dev server (renderer only, no tray)              |
| `npm run tauri dev`                                                        | Full app with hot-reload on both Rust and React sides |
| `npm run lint`                                                             | ESLint + Prettier on the renderer                     |
| `npm run typecheck`                                                        | TypeScript without emit                               |
| `cd src-tauri && cargo clippy --all-targets --all-features -- -D warnings` | Rust linter (matches CI)                              |
| `cd src-tauri && cargo test --all-features`                                | Rust unit + integration tests (matches CI)            |
| `npm run tauri build`                                                      | Production build for the current host platform        |

### Tauri config notes

The popover, onboarding, and settings windows are normal decorated windows with an opaque background — vibrancy/Mica was tried and shelved (it interfered with the auto-fit-to-content sizing and made the small popover read as "ghosty" when nothing was behind it). The popover anchors to the bottom-right of the work area; settings/onboarding centre within the usable area so neither slips behind the taskbar.

The tray is built **only in code** (`src-tauri/src/tray.rs`) — `tauri.conf.json` must **not** also declare a `trayIcon`, or two icons appear in the system tray.

#### Regenerating icons

Tray icons (state-dependent, rasterized from SVG sources in `assets/tray/`):

```bash
python3 scripts/build_tray_icons.py
```

App bundle icons (from the brand mark SVG):

```bash
rsvg-convert -w 1024 -h 1024 \
  assets/headroom-app-icon.svg -o /tmp/headroom-source.png
npx tauri icon /tmp/headroom-source.png
rm /tmp/headroom-source.png
```

Both workflows require `rsvg-convert` (librsvg):

```bash
brew install librsvg
```

---

## Documentation

- [SPEC.md](SPEC.md) — architecture, data sources, auth flows
- [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md) — colors, typography, components, icons
- [CHANGELOG.md](CHANGELOG.md) — release history
- [CONTRIBUTING.md](CONTRIBUTING.md) — code style, PR process
- [FAQ.md](FAQ.md) — common questions about setup, privacy, scope, troubleshooting
- [SECURITY.md](SECURITY.md) — security policy + how to report a vulnerability

---

## Acknowledgments

Headroom exists because the data acquisition problem had already been solved by other widgets we studied:

- [SlavomirDurej/claude-usage-widget](https://github.com/SlavomirDurej/claude-usage-widget) — Electron implementation, embedded webview auth pattern
- [rishi-banerjee1/claude-usage-widget](https://github.com/rishi-banerjee1/claude-usage-widget) — Swift single-file approach, Cloudflare retry logic
- [bristena-op/copilot-usage-tracker](https://github.com/bristena-op/copilot-usage-tracker) — confirmed the original GitHub billing API endpoint for premium requests
- [kasuken/vscode-copilot-insights](https://github.com/kasuken/vscode-copilot-insights) — pointed us at the `copilot_internal/user` endpoint and its AI-Credits quota shape

Headroom's contribution is combining both services in one native menu bar app, with multiple auth paths and a cohesive design.

---

<p align="center">
  <sub>Crafted with <span aria-label="love">❤️</span> and AI by
  <a href="https://github.com/allandecastro">Allan De Castro</a></sub>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-green.svg?style=flat-square" alt="License: MIT" /></a>
</p>
