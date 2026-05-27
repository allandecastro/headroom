import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { TokenCard } from './components/TokenCard';
import { openSettings, getSettings } from './lib/ipc';
import type { Settings } from './lib/ipc';
import { useFitWindowHeight } from './lib/useFitWindow';
import type { Snapshot } from './lib/api';

export default function App() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [showClaudeDesign, setShowClaudeDesign] = useState(false);
  const [, forceTick] = useState(0);
  const bodyRef = useRef<HTMLDivElement>(null);
  useFitWindowHeight(bodyRef, 360, true);

  useEffect(() => {
    // Initial fetch
    invoke<Snapshot>('refresh_all').then(setSnapshot).catch(console.error);
    getSettings()
      .then((s) => setShowClaudeDesign(s.show_claude_design))
      .catch(console.error);

    // Subscribe to backend updates
    const unlistenTokens = listen<Snapshot>('tokens-updated', (event) => {
      setSnapshot(event.payload);
    });
    // React instantly to settings changes (e.g. the Claude Design toggle).
    const unlistenSettings = listen<Settings>('settings-updated', (event) => {
      setShowClaudeDesign(event.payload.show_claude_design);
    });

    // Tick once per second so countdowns update in the UI
    const tick = setInterval(() => forceTick((n) => n + 1), 1000);

    return () => {
      unlistenTokens.then((fn) => fn());
      unlistenSettings.then((fn) => fn());
      clearInterval(tick);
    };
  }, []);

  const polledAgoSec = snapshot ? Math.floor((Date.now() - snapshot.polled_at * 1000) / 1000) : 0;

  return (
    <div className="min-h-screen bg-window-opaque text-fg-primary">
      <div ref={bodyRef} className="px-3.5 pt-3.5 pb-2.5">
        {!snapshot ? (
          <div className="text-xs text-fg-tertiary">Loading…</div>
        ) : (
          <>
            {snapshot.services.map((svc) => (
              <TokenCard key={svc.id} service={svc} showClaudeDesign={showClaudeDesign} />
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
