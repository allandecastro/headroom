# Roadmap

Phased delivery plan for Headroom. Each phase is a shippable cut.

Legend: ✅ done · 🟡 partial · 🔵 deferred (intentional) · ⏳ planned

## Phase 1 — MVP ✅

A working menu bar app that polls Claude Code and Copilot quotas and shows them in the designed popover.

- ✅ Tauri 2 project scaffolding with React + Tailwind renderer
- ✅ Onboarding flow with paste-only auth for both services
- ✅ Keychain credential storage via the `keyring` crate
- ✅ `sources::copilot` adapter calling the official GitHub billing API
- ✅ `sources::claude` adapter calling `claude.ai/api/.../usage`
- ✅ Orchestrator with a 30s polling loop, timeouts, and `classify()` mapping each `SourceError` to a `ServiceState`
- ✅ Tray icon with four state PNGs (ok / warn / crit / unreachable); colour follows the user's configured notification thresholds
- ✅ Popover: service cards, quota rows, status colors, reset countdowns, single-instance lock, autostart, click-to-expand trend chart
- ✅ Settings window: poll interval, theme, notification sliders, Claude Design toggle, per-service Re-auth/Sign-out, Copilot plan change
- ✅ Persisted JSONL usage history (5-min sampling, 30-day retention)
- ✅ CI: lint + typecheck + clippy + cargo test on every PR (44 tests)
- ✅ Build pipeline producing unsigned MSI/dmg/AppImage/deb
- ⏳ Tagged release **v0.1.0** with downloadable artifacts (in progress)

---

## Phase 2 — Smart auth and burndown 🟡

Make onboarding feel like a real product. Add the burndown view that turns Headroom from "passive monitor" into "decision support."

- ✅ Embedded `WebviewWindow` for Claude sign-in (sessionKey captured automatically)
- ✅ Burndown projection — always-visible "On track · ~N% by reset" line, escalates to "On track to exceed · full in Xd" when pacing past the cap
- ✅ Persisted usage history feeding the popover sparkline (downsampled, click-to-expand)
- ✅ Notifications at user-configured thresholds (orange "heads-up" / red "critical"), fired once per crossing
- ✅ Re-authentication flow — `auth_required` state surfaces "Sign in again" + a Re-auth button in Settings
- 🔵 GitHub device flow for Copilot sign-in — **dropped**. The billing endpoint requires fine-grained PAT permissions that classic OAuth scopes can't grant, and shipping a public GitHub App for this one call would add maintainer + phishing surface. The PAT path is one click to "Create one on GitHub →" — same step count, simpler.
- 🔵 Native macOS vibrancy / Windows 11 Mica — **dropped**. Conflicted with the auto-fit-to-content sizing and made small popovers read as "ghosty". Opaque windows are simpler and look better at our sizes.
- ⏳ Pre-onboarding silent scan (detect existing Claude / Copilot OAuth on disk) — nice-to-have, low priority

---

## Phase 3 — Copilot AI Credits migration ⏳

Adapt to GitHub's June 1, 2026 transition from premium requests to AI Credits.

- ⏳ Verify the post-transition endpoint shape (likely the same URL, different response fields)
- ⏳ Update `sources::copilot` to handle both `requests` and `credits` units (the `unit` field on `Quota` already supports this — `UsdCredits` variant is in place)
- ⏳ UI: switch the monthly row to render `$X.XX / $39.00` when the response is in credits
- ⏳ Settings: indicate billing mode (auto-detected)
- ⏳ Migration banner in popover for one week post-transition

---

## Phase 4 — More services ⏳

Add the other AI coding assistants people pay for. Each is a single `QuotaSource` impl following the same pattern as `claude.rs` / `copilot.rs`.

- ⏳ **Cursor** Pro quota (endpoint discovery needed — Cursor exposes usage in-app)
- ⏳ **Codex** (OpenAI's coding agent) — once the usage API is public
- ⏳ **Gemini** (Google) — Code Assist subscription, public billing endpoint
- ⏳ **Perplexity** Pro — research/coding queries, billing API
- ⏳ Multiple accounts per service (e.g. personal + work Claude)

**Architectural pattern:** add a file under `src-tauri/src/sources/`, register it in `lib.rs::AppState::sources`, add tests with wiremock. No frontend changes needed — the popover renders whatever the snapshot contains.

---

## Phase 5 — Distribution and trust ⏳

Make Headroom installable by people who aren't comfortable with `xattr -cr`.

- ⏳ Apple Developer ID code signing + notarization (macOS)
- ⏳ EV / Trusted-publisher code signing (Windows)
- ⏳ In-app auto-updater
- ⏳ Homebrew cask (macOS)
- ⏳ winget manifest (Windows)
- ⏳ Flathub package (Linux)

This phase has real money attached (Apple Developer Program $99/yr, EV cert ~$200/yr). Park it until there are enough non-developer users to justify the cost.

---

## Stretch ideas (no phase yet)

- **Daily / weekly digest** — local notification or in-popover summary of the week's usage trends
- **Slack / Discord webhook notifications** (opt-in) — for team awareness
- **Cost projection in $** — based on overage pricing (e.g. Copilot $0.04/req)
- **Pomodoro integration** — flag when a focus session is about to push you over a threshold

---

## Explicit non-goals

These have been considered and **deliberately dropped** — not "not yet", but "no":

- **CLI / `headroom status --json`** — Headroom is a tray app; a CLI is a different product. Use the orchestrator's persisted snapshot directly if you need scripting.
- **"Bring your own API" generic source** — too much surface for too little value; each source should be a proper adapter with tests.
- **Web sync of usage history** — local-first, no backend, no accounts. Period.
- **Team / org rollups** — Headroom is a personal tool; team billing dashboards are a different product.
- **Self-hosted backend / public dashboard** — would mean owning user data; not happening.
