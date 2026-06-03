# Screenshot tooling

Regenerates the README popover screenshots from the **real** React components, so
the docs stay in sync with the UI.

```bash
npx playwright install chromium   # one-time: fetch the browser
npm run screenshots               # render + write docs/screenshots/*.png
```

`capture.mjs` boots Vite programmatically, loads `harness.html` (which renders
`TokenCard` with canned `ServiceStatus` data — no Tauri backend), and screenshots
each state with Playwright. The clock, timezone, and locale are pinned so the
output is byte-identical across runs and machines.

- **Add a state:** add an entry to `STATES` in `harness.tsx` and a row in `SHOTS`
  in `capture.mjs`.
- **CI:** `.github/workflows/screenshots.yml` runs this on every PR and commits
  any changes back to the branch automatically.
