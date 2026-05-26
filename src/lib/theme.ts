// Applies the color theme by toggling a `.dark` class on <html>.
// Driven by the persisted `theme` setting; 'auto' follows the OS preference.
// Re-applies live when the OS theme flips (auto) or settings change in any window.

import { listen } from '@tauri-apps/api/event';
import { getSettings } from './ipc';
import type { Settings } from './ipc';

type Theme = Settings['theme'];

const darkQuery = window.matchMedia('(prefers-color-scheme: dark)');

function isDark(theme: Theme): boolean {
  if (theme === 'dark') return true;
  if (theme === 'light') return false;
  return darkQuery.matches;
}

function apply(theme: Theme): void {
  document.documentElement.classList.toggle('dark', isDark(theme));
}

let current: Theme = 'auto';

/**
 * Apply the theme immediately (OS preference, to avoid a flash), then refine
 * from the persisted setting and keep it in sync with OS + settings changes.
 */
export async function initTheme(): Promise<void> {
  apply(current); // synchronous best-guess before first paint

  darkQuery.addEventListener('change', () => {
    if (current === 'auto') apply(current);
  });

  await listen<Settings>('settings-updated', (event) => {
    current = event.payload.theme;
    apply(current);
  });

  try {
    current = (await getSettings()).theme;
    apply(current);
  } catch {
    // Backend not ready or no settings file yet — 'auto' is a fine fallback.
  }
}
