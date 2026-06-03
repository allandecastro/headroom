# Screenshot tooling

Regenerates the README popover screenshots from the **real** React components, so
the docs stay in sync with the UI.

```bash
npx playwright install chromium   # one-time: fetch the browser
npm run screenshots               # render + write docs/screenshots/*.png
```

`capture.mjs` boots Vite programmatically, loads `harness.html`, and screenshots
each state with Playwright. The clock, timezone, and locale are pinned so the
output is byte-identical across runs on a given OS. Two kinds of state:

- **card states** (`quota`, `pooled`) render a single popover `TokenCard`, captured
  by its `#shot` element.
- **window states** (`popover`, `settings`, `onboarding`) render the real full
  window components (`App` / `SettingsPanel` / `OnboardingFlow`) with their Tauri
  IPC mocked (`@tauri-apps/api/mocks`), clipped to the content height. The mock
  data (snapshot, settings, connected accounts) lives at the top of `harness.tsx`.

- **Add a state:** add an entry to `CARD_STATES` (or a window branch) in
  `harness.tsx` and a row to `SHOTS` in `capture.mjs`.
- **CI:** `.github/workflows/screenshots.yml` runs this on every PR and **fails
  if the committed screenshots are stale** — so when the UI changes, run
  `npm run screenshots` and commit the result. (Verify-only rather than
  auto-commit, so you review the image diffs and there's no write token to manage.)
