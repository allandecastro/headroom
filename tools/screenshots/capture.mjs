// Regenerate the README screenshots deterministically.
//
// Boots Vite programmatically (so the real React components + Tailwind render),
// drives headless Chromium via Playwright, and writes one PNG per state. Run
// with `npm run screenshots`.
//
// Card states screenshot the #shot element; window states (the real App /
// Settings / Onboarding components, which fill the window) are clipped to the
// rendered content height.
//
// Prereq: the Chromium browser binary — `npx playwright install chromium`.
import { createServer } from 'vite';
import { chromium } from 'playwright';

const SHOTS = [
  { state: 'quota', out: 'docs/screenshots/copilot-business-quota.png', width: 360 },
  { state: 'pooled', out: 'docs/screenshots/copilot-business-pooled.png', width: 360 },
  { state: 'popover', out: 'docs/screenshots/widget.png', width: 360, window: true },
  { state: 'settings', out: 'docs/screenshots/settings.png', width: 480, window: true },
  { state: 'onboarding', out: 'docs/screenshots/setup.png', width: 480, window: true },
];

const server = await createServer({
  server: { port: 5599, strictPort: false },
  logLevel: 'warn',
});
await server.listen();
const base = server.resolvedUrls.local[0].replace(/\/$/, '');
console.log(`harness server: ${base}`);

const browser = await chromium.launch();
try {
  for (const { state, out, width, window: isWindow } of SHOTS) {
    // Per-shot page: fixed width, frozen clock + timezone + locale so the render
    // is byte-identical across runs (the card shows clock-relative reset text).
    const page = await browser.newPage({
      deviceScaleFactor: 2,
      timezoneId: 'UTC',
      locale: 'en-US',
      viewport: { width, height: 1800 },
    });
    await page.clock.install({ time: new Date('2026-06-01T09:00:00Z') });
    await page.goto(`${base}/tools/screenshots/harness.html?state=${state}`, { waitUntil: 'load' });

    if (isWindow) {
      // Real window component — clip to the inner content element's height so
      // the shot is tight (the outer element is full window height).
      await page.waitForFunction(
        () => document.querySelector('#root')?.firstElementChild?.firstElementChild,
      );
      await page.waitForTimeout(600); // let mocked data resolve + re-render
      const height = await page.evaluate(() =>
        Math.ceil(
          document
            .querySelector('#root')
            .firstElementChild.firstElementChild.getBoundingClientRect().bottom,
        ),
      );
      await page.screenshot({ path: out, clip: { x: 0, y: 0, width, height } });
    } else {
      await page.waitForSelector('#shot');
      await page.waitForTimeout(400);
      await page.locator('#shot').screenshot({ path: out });
    }
    await page.close();
    console.log(`wrote ${out}`);
  }
} finally {
  await browser.close();
  await server.close();
}
