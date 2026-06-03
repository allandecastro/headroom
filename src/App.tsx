import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-shell';
import { TokenCard } from './components/TokenCard';
import { openSettings, getSettings, getUpdate, installUpdate } from './lib/ipc';
import type { Settings, UpdateProgress } from './lib/ipc';
import { useFitWindowHeight } from './lib/useFitWindow';
import type { Snapshot, UpdateInfo } from './lib/api';

export default function App() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [showClaudeDesign, setShowClaudeDesign] = useState(false);
  // Tray + popover colour thresholds, sourced from the same setting as the
  // desktop notifications so one knob controls everything.
  const [warnPct, setWarnPct] = useState(80);
  const [critPct, setCritPct] = useState(95);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [updateDismissed, setUpdateDismissed] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [installPct, setInstallPct] = useState<number | null>(null);
  const [installError, setInstallError] = useState(false);
  const [, forceTick] = useState(0);
  const bodyRef = useRef<HTMLDivElement>(null);
  useFitWindowHeight(bodyRef, 360, true);

  useEffect(() => {
    // Initial fetch
    invoke<Snapshot>('refresh_all').then(setSnapshot).catch(console.error);
    getSettings()
      .then((s) => {
        setShowClaudeDesign(s.show_claude_design);
        setWarnPct(s.notify_warn_pct);
        setCritPct(s.notify_crit_pct);
      })
      .catch(console.error);
    getUpdate().then(setUpdate).catch(console.error);

    // Subscribe to backend updates
    const unlistenTokens = listen<Snapshot>('tokens-updated', (event) => {
      setSnapshot(event.payload);
    });
    // A newer release was found by the background checker.
    const unlistenUpdate = listen<UpdateInfo>('update-available', (event) => {
      setUpdate(event.payload);
      setUpdateDismissed(false);
    });
    // React instantly to settings changes (Claude Design toggle, threshold sliders).
    const unlistenSettings = listen<Settings>('settings-updated', (event) => {
      setShowClaudeDesign(event.payload.show_claude_design);
      setWarnPct(event.payload.notify_warn_pct);
      setCritPct(event.payload.notify_crit_pct);
    });
    // Download progress while an in-app update installs.
    const unlistenProgress = listen<UpdateProgress>('update-progress', (event) => {
      const { downloaded, content_length } = event.payload;
      setInstallPct(
        content_length ? Math.min(100, Math.round((downloaded / content_length) * 100)) : null,
      );
    });

    // Tick once per second so countdowns update in the UI
    const tick = setInterval(() => forceTick((n) => n + 1), 1000);

    return () => {
      unlistenTokens.then((fn) => fn());
      unlistenSettings.then((fn) => fn());
      unlistenUpdate.then((fn) => fn());
      unlistenProgress.then((fn) => fn());
      clearInterval(tick);
    };
  }, []);

  // "Update now": download + install + relaunch. On success the app restarts and
  // this never returns; on an unsupported platform / failure we open the release
  // page so the manual download still works.
  async function handleUpdateNow() {
    if (!update) return;
    setInstalling(true);
    setInstallError(false);
    setInstallPct(null);
    try {
      const outcome = await installUpdate();
      if (outcome?.kind === 'open_url') {
        open(outcome.url).catch(console.error);
        setInstalling(false);
      }
    } catch (e) {
      console.error(e);
      setInstalling(false);
      setInstallError(true);
    }
  }

  const polledAgoSec = snapshot ? Math.floor((Date.now() - snapshot.polled_at * 1000) / 1000) : 0;

  return (
    <div className="min-h-screen bg-window-opaque text-fg-primary">
      <div ref={bodyRef} className="px-3.5 pt-3.5 pb-2.5">
        {!snapshot ? (
          <div className="text-xs text-fg-tertiary">Checking usage…</div>
        ) : (
          <>
            {update && !updateDismissed && (
              <div className="mb-2.5 flex items-center justify-between gap-2 rounded-[6px] bg-black/[0.04] px-2.5 py-1.5 dark:bg-white/[0.06]">
                <span className="text-[11px] text-fg-secondary">
                  <span aria-hidden className="text-state-ok-text dark:text-state-ok-text-dark">
                    ⬆
                  </span>{' '}
                  {installError ? 'Update failed' : `Headroom v${update.version} available`}
                </span>
                <span className="flex items-center gap-2.5">
                  {installing ? (
                    <span className="text-[11px] text-fg-tertiary">
                      {installPct !== null ? `Downloading… ${installPct}%` : 'Installing…'}
                    </span>
                  ) : installError ? (
                    <button
                      onClick={() => open(update.url).catch(console.error)}
                      className="text-[11px] font-medium text-state-ok-text hover:underline dark:text-state-ok-text-dark"
                    >
                      Open page
                    </button>
                  ) : (
                    <button
                      onClick={handleUpdateNow}
                      className="text-[11px] font-medium text-state-ok-text hover:underline dark:text-state-ok-text-dark"
                    >
                      Update now
                    </button>
                  )}
                  {!installing && (
                    <button
                      aria-label="Dismiss update notice"
                      onClick={() => setUpdateDismissed(true)}
                      className="text-[12px] leading-none text-fg-tertiary hover:text-fg-secondary"
                    >
                      ✕
                    </button>
                  )}
                </span>
              </div>
            )}
            {snapshot.services.map((svc) => (
              <TokenCard
                key={svc.id}
                service={svc}
                showClaudeDesign={showClaudeDesign}
                warnPct={warnPct}
                critPct={critPct}
              />
            ))}

            <footer className="mt-2 flex items-center justify-between pt-1 text-[12px] text-fg-tertiary">
              <button
                onClick={() => invoke<Snapshot>('refresh_all').then(setSnapshot)}
                className="inline-flex items-center gap-1.5 hover:text-fg-secondary"
              >
                <span aria-hidden className="text-[16px] leading-none">
                  ↻
                </span>
                {polledAgoSec}s ago
              </button>
              <span className="flex gap-4">
                <button
                  aria-label="Settings"
                  onClick={() => openSettings().catch(console.error)}
                  className="text-[17px] leading-none hover:text-fg-secondary"
                >
                  ⚙
                </button>
                <button
                  aria-label="Quit"
                  onClick={() => invoke('quit_app')}
                  className="text-[17px] leading-none hover:text-fg-secondary"
                >
                  ⏻
                </button>
              </span>
            </footer>
          </>
        )}
      </div>
    </div>
  );
}
