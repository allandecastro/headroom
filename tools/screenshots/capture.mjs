// Regenerate the README popover screenshots deterministically.
//
// Boots Vite programmatically (so the real React components + Tailwind render),
// drives headless Chromium via Playwright, and writes one PNG per state to
// docs/screenshots/. Run with `npm run screenshots`.
//
// Prereq: the Chromium browser binary — `npx playwright install chromium`
// (CI runs `npx playwright install --with-deps chromium`).
import { createServer } from 'vite';
import { chromium } from 'playwright';

// state (harness ?state=) → output PNG
const SHOTS = [
  ['quota', 'docs/screenshots/copilot-business-quota.png'],
  ['pooled', 'docs/screenshots/copilot-business-pooled.png'],
  ['multi-account', 'docs/screenshots/copilot-multi-account.png'],
];

const server = await createServer({
  // A non-default port so this never clashes with a running `npm run dev`.
  server: { port: 5599, strictPort: false },
  logLevel: 'warn',
});
await server.listen();
const base = server.resolvedUrls.local[0].replace(/\/$/, '');
console.log(`harness server: ${base}`);

const browser = await chromium.launch();
try {
  // Pin scale, timezone, and locale so the render is identical on any machine —
  // the card shows clock-relative reset text, which would otherwise drift every
  // run (and make the auto-commit workflow churn).
  const page = await browser.newPage({
    deviceScaleFactor: 2,
    timezoneId: 'UTC',
    locale: 'en-US',
  });
  // Freeze the clock so `Date.now()` / `new Date()` in the page are constant —
  // the reset countdown becomes a fixed "in 27d 15h" instead of live.
  await page.clock.install({ time: new Date('2026-06-01T09:00:00Z') });
  for (const [state, out] of SHOTS) {
    await page.goto(`${base}/tools/screenshots/harness.html?state=${state}`, {
      waitUntil: 'load',
    });
    await page.waitForSelector('#shot');
    await page.waitForTimeout(400); // let fonts settle
    await page.locator('#shot').screenshot({ path: out });
    console.log(`wrote ${out}`);
  }
} finally {
  await browser.close();
  await server.close();
}
