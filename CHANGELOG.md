# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial project scaffold: Tauri 2 + React + Tailwind
- `QuotaSource` trait with Claude and Copilot adapters
- Keychain credentials wrapper
- Tray icon state machine (ok / warn / crit / unreachable)
- Popover UI with translucent surface
- CI pipeline (lint, typecheck, clippy, multi-platform build)
- Release pipeline triggered by `v*` tags
- Tray icon generator script (`scripts/build_tray_icons.py`)

### Known limitations
- Auth flows are paste-only in this scaffold; embedded webview and GitHub device flow land in Phase 2 (see [ROADMAP.md](ROADMAP.md))
- The Claude usage response shape is tentative; will be refined against real API responses during implementation
- No code signing yet; macOS users need `xattr -cr` on first launch
