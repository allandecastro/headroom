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

### Why does Copilot need a Personal Access Token instead of "Sign in with GitHub"?

GitHub's billing endpoint (`/users/{u}/settings/billing/premium_request/usage`) requires a fine-grained permission (`Account → Plan: Read-only`) that classic OAuth scopes can't grant. We tried the OAuth device flow — it works for sign-in but returns 404 on the billing call. Shipping a public GitHub App for one HTTP call would add maintainer + phishing surface for negligible UX gain. So we use the PAT path, just like every other working third-party Copilot widget. The onboarding form has a one-click _"Create one on GitHub →"_ button that takes you straight to the right page.

### Where do I find my Copilot plan?

Visit [github.com/settings/copilot](https://github.com/settings/copilot) — your plan (Free / Pro / Pro+ / Business / Enterprise) is shown at the top. If you guessed wrong at onboarding, change it inline at **Settings → Services → Copilot plan** in Headroom — no re-auth needed.

### Where do I find my Claude session key?

Easiest path: click **Sign in with Claude** in onboarding — the embedded webview signs you in and captures the cookie automatically. If that doesn't work (some Linux distros lack the right webkit, some identity providers behave oddly inside webviews), use the paste path: in your browser, sign in to `claude.ai`, open DevTools → Application → Cookies → `claude.ai` → copy the value of `sessionKey`.

## Troubleshooting

### My tray icon is hidden!

On Windows 11, new tray icons land in the hidden overflow under the `^` chevron near the clock. Click `^`, find Headroom, drag it onto the always-visible taskbar.

### Notifications say "Windows PowerShell" instead of "Headroom".

This happens when running a loose `.exe` launched from a terminal — Windows attributes toast notifications to the launching process's AppUserModelID. The **installed MSI** registers Headroom's own AUMID via the Start Menu shortcut and correctly attributes toasts to "Headroom". Install the MSI from [Releases](https://github.com/allandecastro/headroom/releases) instead of running a development build.

### My Copilot card says "Couldn't fetch usage · HTTP 404".

Almost always means the PAT lacks the right permission. Regenerate it at [github.com/settings/personal-access-tokens/new](https://github.com/settings/personal-access-tokens/new) with **Account → Plan: Read-only** and paste the new one. Use the "Create one on GitHub →" button in the onboarding form — it deep-links to the right page.

### The trend sparkline says "collecting…".

Headroom records one history sample per quota every ~5 minutes. A fresh install needs at least two samples in the current window before the sparkline can draw, so give it ~10 minutes. History persists across restarts, so you only wait once.

### Why doesn't the popover show daily/hourly Copilot rate limits?

GitHub doesn't expose them. The only public Copilot billing endpoint reports **monthly** totals; the per-session and weekly rate-limit windows GitHub enforces aren't documented numerically and have no API to query. What Headroom _can_ (and does) show is the **recent burn rate** — _"Burning N%/day · M%/day keeps you on track"_ — derived locally from your own history samples. It can't tell you you're throttled right now, but it can tell you you're about to be.

### My tray icon turned grey but I'm not "unreachable".

The grey icon means _all_ services are in an error state (unreachable / auth_required) with nothing active to colour from. If at least one service is healthy, its quota colour drives the tray instead. A grey tray usually means: token expired (re-auth), network blip (will retry), or your laptop just woke from sleep.

## Future

### Will you support Cursor / Codex / Gemini / Perplexity?

Yes — Phase 4 of the [roadmap](ROADMAP.md). Each is a new file under `src-tauri/src/sources/` implementing the `QuotaSource` trait. PRs welcome.

### Auto-update? Code signing?

Phase 5 of the [roadmap](ROADMAP.md). Code signing has real money attached (~$99/yr Apple, ~$200/yr Windows EV cert), so it'll wait until there are enough non-developer users to justify it.

### Headroom CLI? "Bring your own API" source?

Explicitly **no** — see the _non-goals_ section of the [roadmap](ROADMAP.md). Headroom is a focused tray app, not a Swiss-army knife.
