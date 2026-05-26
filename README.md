<p>
  <img src="assets/headroom-mark.svg" width="48" alt="Headroom" />
</p>

# Headroom

> Know your headroom. A menu bar app that tracks your Claude Code and GitHub Copilot quotas before you hit them.

![CI](https://github.com/allandecastro/headroom/actions/workflows/ci.yml/badge.svg)

Headroom sits in your menu bar and shows, at a glance, how much of your AI coding assistant budget you have left — across the rolling 5-hour window, the 7-day weekly cap, and the monthly Copilot allowance. It tells you when to switch from Opus to Sonnet, how long until the next reset, and projects whether you'll make it to Monday at your current pace.

It is built for developers on Claude Pro/Max and Copilot Pro/Pro+ who actually use these tools all day and have run into the "usage limit reached" wall mid-task.

---

## Screenshots

_To be added once the first build ships. See [`docs/mockups/`](docs/mockups/) for the design references._

---

## Features

- **Live quota tracking** for Claude Code (5-hour, weekly Sonnet, weekly Opus) and GitHub Copilot (monthly premium requests, soon AI Credits)
- **Tray badge** showing the worst quota across services, color-coded green / amber / red
- **7-day burndown chart** with linear projection — see whether you'll hit the cap before reset
- **Native vibrancy** on macOS and Mica on Windows 11 — the popover feels like part of the OS, not an Electron window
- **Multi-auth onboarding** — sign in via embedded webview / OAuth device flow, or paste a token for power users
- **Credentials in OS keychain** only — nothing leaves your machine
- **Source detection** — if Claude Code or Copilot is already authenticated on your machine, Headroom finds it and uses it

---

## Status

Pre-alpha. The data acquisition strategies for both Claude and Copilot are validated against working third-party widgets. Phase 1 (MVP) targets a single-user, single-account build with manual token entry as the fallback auth path. See [ROADMAP.md](ROADMAP.md) for what's in each phase.

---

## Install

### From a release

Pre-built binaries will be published on the [Releases](https://github.com/USER/headroom/releases) page once the first tagged version ships. Both signed installers (macOS `.dmg`, Windows MSI) and Linux `AppImage` will be available.

### From source

You need Rust (1.78+) and Node.js (20+) installed.

```bash
git clone https://github.com/USER/headroom.git
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
│   ├── components/         # TokenCard, SettingsPanel, OnboardingFlow, etc.
│   ├── lib/                # Tauri IPC wrappers, formatters
│   ├── App.tsx
│   └── main.tsx
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── sources/        # QuotaSource trait and implementations
│   │   ├── credentials.rs  # OS keychain wrapper
│   │   ├── tray.rs         # Tray icon state machine
│   │   ├── lib.rs          # App builder, IPC handlers, poll loop
│   │   └── main.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── docs/                   # Design references, mockups, spec extras
└── .github/workflows/      # CI + release pipelines
```

### Useful commands

| Command                       | What it does                                              |
| ----------------------------- | --------------------------------------------------------- |
| `npm run dev`                 | Vite dev server (renderer only, no tray)                  |
| `npm run tauri dev`           | Full app with hot-reload on both Rust and React sides     |
| `npm run lint`                | ESLint + Prettier on the renderer                         |
| `npm run typecheck`           | TypeScript without emit                                   |
| `cd src-tauri && cargo clippy`| Rust linter                                               |
| `cd src-tauri && cargo test`  | Rust unit + integration tests                             |
| `npm run tauri build`         | Production build for the current host platform            |

### Tauri config notes

`src-tauri/tauri.conf.json` sets `transparent: true` on the main popover window and requests `vibrancy: "sidebar"` on macOS / `effects: ["mica"]` on Windows. Linux falls back to a translucent solid since blur support varies by compositor.

---

## Documentation

- [SPEC.md](SPEC.md) — architecture, data sources, auth flows
- [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md) — colors, typography, components, icons
- [ROADMAP.md](ROADMAP.md) — phased delivery plan
- [CONTRIBUTING.md](CONTRIBUTING.md) — code style, PR process
- [AGENTS.md](AGENTS.md) — guidelines for AI coding assistants working on the project

---

## Acknowledgments

Headroom exists because the data acquisition problem had already been solved by other widgets we studied:

- [SlavomirDurej/claude-usage-widget](https://github.com/SlavomirDurej/claude-usage-widget) — Electron implementation, embedded webview auth pattern
- [rishi-banerjee1/claude-usage-widget](https://github.com/rishi-banerjee1/claude-usage-widget) — Swift single-file approach, Cloudflare retry logic
- [bristena-op/copilot-usage-tracker](https://github.com/bristena-op/copilot-usage-tracker) — confirmed the official GitHub billing API endpoint for premium requests

Headroom's contribution is combining both services in one native menu bar app, with multiple auth paths and a cohesive design.

---

## License

MIT — see [LICENSE](LICENSE).
