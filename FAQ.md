# FAQ

Common questions about Headroom. If something's missing, [open an issue](https://github.com/allandecastro/headroom/issues/new).

## Privacy & data

### Does Headroom send any data anywhere?

No. Headroom is local-only: no telemetry, no analytics, no backend. The only network calls it makes are to your AI vendors' own APIs (`claude.ai`, `api.github.com`) to fetch _your_ usage, using _your_ credentials. Everything else lives on your machine.

### Where are my credentials stored?

In your operating system's keychain (Windows Credential Manager / macOS Keychain / Linux Secret Service), via the [`keyring`](https://crates.io/crates/keyring) Rust crate. They're encrypted at rest by the OS. Headroom reads them only when making an HTTP request to the corresponding service; the values are never logged or persisted anywhere else.

### Where are my settings and usage history stored?

Plain-text files in your user data dir:

- **Settings:** `%APPDATA%\headroom\settings.json` (Windows) · `~/Library/Application Support/headroom/settings.json` (macOS) · `~/.config/headroom/settings.json` (Linux)
- **History:** `history.jsonl` in the corresponding data dir (`%APPDATA%\headroom\`, `~/Library/Application Support/headroom/`, `~/.local/share/headroom/`)

History is sampled every ~5 minutes (one line per quota), pruned to 30 days. Each line is `{ts, service, window, used_pct}` — no tokens, no PII, just percentages.

### Is any of this a security risk?

Credentials use the same OS keychain that 1Password, VS Code, and GitHub CLI use — about as well-protected as a desktop app can manage. Settings and history are non-sensitive. The realistic threat model is "malware running as your user on your machine" — which is a problem for every desktop app, not just Headroom. See [SECURITY.md](SECURITY.md) for the full policy.

## Setup

### What token does Copilot need?

Any GitHub personal access token — classic or fine-grained, no specific permission required. Headroom reads your Copilot quota from `copilot_internal/user`, which accepts a plain GitHub token, so there's nothing special to enable. Paste it in the onboarding form (or use the one-click _"Create one on GitHub →"_ button). Earlier versions needed a fine-grained PAT with `Account → Plan: Read-only`; that's no longer the case — a token created before this change keeps working.

### Where do I find my Copilot plan?

You don't need to — Headroom reads your plan, quota cap, and reset date straight from your account, so it stays correct even if you change tiers. To see it yourself, visit [github.com/settings/copilot](https://github.com/settings/copilot).

### Where do I find my Claude session key?

Easiest path: click **Sign in with Claude** in onboarding — the embedded webview signs you in and captures the cookie automatically. If that doesn't work (some Linux distros lack the right webkit, some identity providers behave oddly inside webviews), use the paste path: in your browser, sign in to `claude.ai`, open DevTools → Application → Cookies → `claude.ai` → copy the value of `sessionKey`.

## Troubleshooting

### My tray icon is hidden!

On Windows 11, new tray icons land in the hidden overflow under the `^` chevron near the clock. Click `^`, find Headroom, drag it onto the always-visible taskbar.

### Notifications say "Windows PowerShell" instead of "Headroom".

This happens when running a loose `.exe` launched from a terminal — Windows attributes toast notifications to the launching process's AppUserModelID. The **installed MSI** registers Headroom's own AUMID via the Start Menu shortcut and correctly attributes toasts to "Headroom". Install the MSI from [Releases](https://github.com/allandecastro/headroom/releases) instead of running a development build.

### My Copilot card says it couldn't fetch usage.

`copilot_internal/user` is an internal GitHub endpoint, so an `HTTP 401/403` means the token was rejected — regenerate one at [github.com/settings/tokens/new](https://github.com/settings/tokens/new) and paste it again. A `404`/other error usually means GitHub changed or restricted the endpoint for your account type; please [open an issue](https://github.com/allandecastro/headroom/issues) with the status code.

### The trend sparkline says "collecting…".

Headroom records one history sample per quota every ~5 minutes. A fresh install needs at least two samples in the current window before the sparkline can draw, so give it ~10 minutes. History persists across restarts, so you only wait once.

### Why doesn't the popover show daily/hourly Copilot rate limits?

GitHub doesn't expose them. The Copilot endpoint reports **monthly** quota totals; the per-session and weekly rate-limit windows GitHub enforces aren't documented numerically and have no API to query. What Headroom _can_ (and does) show is the **recent burn rate** — _"Burning N%/day · M%/day keeps you on track"_ — derived locally from your own history samples. It can't tell you you're throttled right now, but it can tell you you're about to be.

### My tray icon turned grey but I'm not "unreachable".

The grey icon means _all_ services are in an error state (unreachable / auth_required) with nothing active to colour from. If at least one service is healthy, its quota colour drives the tray instead. A grey tray usually means: token expired (re-auth), network blip (will retry), or your laptop just woke from sleep.

## Scope

### What Headroom doesn't do (by design)

A few things are intentionally **out of scope** to keep Headroom a focused tray app:

- **No CLI / `headroom status --json`** — Headroom is a tray app; a CLI is a different product. If you need to script against your quotas, read the persisted JSONL at `~/.local/share/headroom/history.jsonl` directly.
- **No "bring your own API" generic source** — every supported service is a proper adapter with tests, not a configurable JSONPath probe.
- **No web sync, no accounts, no backend** — local-first; everything lives on your machine.
- **No team / org rollups** — Headroom is a personal tool. Team billing dashboards are a different product.

### Will you support Cursor / Codex / Gemini / Perplexity?

Yes — that's the natural next direction. Each is a new file under `src-tauri/src/sources/` implementing the `QuotaSource` trait. Follow [open issues](https://github.com/allandecastro/headroom/issues?q=is%3Aissue+label%3Aenhancement) or open one for the service you want most. PRs welcome.

### Auto-update? Code signing?

Planned but not free — Apple Developer Program is $99/yr, Windows EV cert ~$200/yr. It'll wait until there are enough non-developer users to justify the cost. Until then, manual updates from the [Releases](https://github.com/allandecastro/headroom/releases) page.
