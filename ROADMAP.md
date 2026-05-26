# Roadmap

Phased delivery plan for Headroom. Each phase is a shippable cut.

## Phase 1 — MVP

**Goal:** A working menu bar app that polls Claude Code and Copilot quotas and shows them in the designed popover. Manual paste auth is acceptable as the primary path; magic auth can come in Phase 2.

**Scope:**

- [ ] Tauri 2 project scaffolding with React + Tailwind renderer
- [ ] Onboarding flow with paste-only auth for both services
- [ ] Keychain credential storage via the `keyring` crate
- [ ] `sources::copilot` adapter calling the official GitHub billing API
- [ ] `sources::claude` adapter calling `claude.ai/api/organizations/{id}/usage`
- [ ] Orchestrator with 30s polling loop and exponential backoff
- [ ] Tray icon with four state PNGs (ok / warn / crit / unreachable)
- [ ] Popover UI: service sections, quota rows, status colors, reset countdowns
- [ ] Settings window: poll interval, theme override, services on/off, plan selector
- [ ] Local JSONL history for the future burndown chart (collect now, render later)
- [ ] CI: lint + typecheck + clippy + cargo test on every PR
- [ ] Build pipeline producing unsigned binaries for macOS, Windows, Linux
- [ ] Tagged release v0.1.0 with downloadable artifacts

**Out of scope for Phase 1:** burndown chart UI, embedded webview auth, GitHub device flow, browser cookie auto-detection, multi-account, notifications.

**Done criteria:** Allan can sign in, leave the app running for an hour, and watch the bars update correctly. Switching from Opus to Sonnet visibly affects the Opus row's burn rate within 30 seconds.

---

## Phase 2 — Smart auth and burndown

**Goal:** Make onboarding feel like a real product. Add the burndown view that turns Headroom from "passive monitor" into "decision support."

**Scope:**

- [ ] Embedded `WebviewWindow` for Claude sign-in (sessionKey captured automatically)
- [ ] GitHub device flow for Copilot sign-in
- [ ] Pre-onboarding silent scan: detect Claude Code OAuth in keychain, Copilot OAuth in `~/.config/github-copilot/apps.json`
- [ ] Burndown chart view (the SVG line chart we designed) with projection to next reset
- [ ] "At this pace, cap hit at X" calculation based on rolling average
- [ ] Notifications at user-configured thresholds (80%, 95%)
- [ ] Native macOS vibrancy and Windows Mica configured in `tauri.conf.json`
- [ ] Re-authentication flow when a token expires (auth_required state)

**Done criteria:** A new user can install Headroom and reach a working dashboard in under 60 seconds without ever opening DevTools.

---

## Phase 3 — Copilot AI Credits migration

**Goal:** Adapt to GitHub's June 1, 2026 transition from premium requests to AI Credits.

**Scope:**

- [ ] Verify new endpoint shape (likely the same URL, different response fields)
- [ ] Update `sources::copilot` to handle both `requests` and `credits` units
- [ ] UI: switch the monthly row to render `$X.XX / $39.00` when on the credits plan
- [ ] Settings: indicate billing mode (auto-detected) for clarity
- [ ] Migration banner in popover for one week post-transition, then auto-dismissed

**Done criteria:** Users on annual plans (still on PRUs) and users on monthly plans (on AI Credits) both see correct data with no manual reconfiguration.

---

## Phase 4 — Beyond Claude + Copilot

**Goal:** Make Headroom the de facto tracker for any AI coding assistant with a usage cap.

**Scope:**

- [ ] Cursor Pro quota integration (endpoint TBD — Cursor exposes some usage in-app, need to find the API path)
- [ ] Codeium / Windsurf integration if their billing exposes a similar endpoint
- [ ] Generic "Bring Your Own API" source that lets the user define a JSONPath into any HTTP response
- [ ] Multiple accounts per service (e.g. personal + work Claude accounts)
- [ ] Per-account aggregation in the tray badge ("worst of any account")

**Done criteria:** Adding a new AI service requires only a new file under `src-tauri/src/sources/` and a single line in the source registry. No UI changes.

---

## Phase 5 — Distribution and trust

**Goal:** Make Headroom installable by people who aren't comfortable with `xattr -cr`.

**Scope:**

- [ ] Apple Developer ID code signing + notarization for macOS builds
- [ ] EV code signing certificate for Windows builds
- [ ] Sparkle / Squirrel auto-updater for in-app upgrades
- [ ] Homebrew cask for Mac installation
- [ ] winget manifest for Windows installation
- [ ] Flathub package for Linux installation

This phase has real money attached (Apple Developer Program is $99/yr, EV cert ~$200/yr). Park it until there are enough non-developer users to justify the cost.

---

## Stretch ideas (no phase yet)

- **Daily/weekly digest emails** summarizing usage trends
- **Slack/Discord webhook notifications** for team awareness
- **Pomodoro integration** — flag when a focus session is about to push you over a threshold
- **Cost projection in $** based on overage pricing (Copilot $0.04/req when enabled)
- **Headroom CLI** — `headroom status --json` for terminal users and CI pipelines
- **Public usage dashboard** (opt-in) — anonymized aggregate stats for "how much Claude Code does a typical Max 5× user burn in a week"
