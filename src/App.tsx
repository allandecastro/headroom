import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { TokenCard } from './components/TokenCard';
import type { Snapshot } from './lib/api';

export default function App() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [, forceTick] = useState(0);

  useEffect(() => {
    // Initial fetch
    invoke<Snapshot>('refresh_all').then(setSnapshot).catch(console.error);

    // Subscribe to backend updates
    const unlisten = listen<Snapshot>('tokens-updated', (event) => {
      setSnapshot(event.payload);
    });

    // Tick once per second so countdowns update in the UI
    const tick = setInterval(() => forceTick((n) => n + 1), 1000);

    return () => {
      unlisten.then((fn) => fn());
      clearInterval(tick);
    };
  }, []);

  if (!snapshot) {
    return (
      <div className="surface p-4 text-fg-tertiary text-xs">
        Loading…
      </div>
    );
  }

  const polledAgoSec = Math.floor((Date.now() - snapshot.polled_at * 1000) / 1000);

  return (
    <div className="surface">
      {snapshot.services.map((svc) => (
        <TokenCard key={svc.id} service={svc} />
      ))}

      <footer className="flex justify-between items-center mt-1 pt-2 border-t border-hairline border-default text-2xs text-fg-tertiary">
        <button
          onClick={() => invoke('refresh_all').then(setSnapshot)}
          className="inline-flex items-center gap-1.5 hover:text-fg-secondary"
        >
          <span aria-hidden>↻</span>
          {polledAgoSec}s ago
        </button>
        <span className="flex gap-3">
          <button aria-label="History" className="hover:text-fg-secondary">⌧</button>
          <button aria-label="Settings" className="hover:text-fg-secondary">⚙</button>
          <button
            aria-label="Quit"
            onClick={() => invoke('quit_app')}
            className="hover:text-fg-secondary"
          >
            ⏻
          </button>
        </span>
      </footer>
    </div>
  );
}
