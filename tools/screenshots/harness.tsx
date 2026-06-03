// Screenshot harness — renders the real popover card with canned data so the
// capture script (capture.mjs) can produce deterministic README screenshots
// without the Tauri backend. Not part of the app build (index.html is the only
// bundled entry). Add a state to STATES, reference it in capture.mjs.
import ReactDOM from 'react-dom/client';
import { TokenCard } from '../../src/components/TokenCard';
import type { ServiceStatus } from '../../src/lib/api';
import '../../src/index.css';

const H = 3_600_000;
const iso = (ms: number) => new Date(Date.now() + ms).toISOString();
const resetAt = iso((27 * 24 + 15) * H);

const STATES: Record<string, ServiceStatus> = {
  // GitHub Copilot Business with a per-user AI-Credits quota assigned.
  quota: {
    id: 'copilot:1',
    name: 'GitHub Copilot',
    plan: 'Business',
    state: 'active',
    quotas: [
      {
        window: 'monthly',
        label: 'AI Credits',
        used: 1280.2,
        total: 2400,
        unit: 'requests',
        resets_at: resetAt,
      },
    ],
    copilot_usage: {
      mode: 'ai_credits_capped',
      label: 'AI Credits',
      entitlement: 2400,
      remaining: 1119.8,
      used: 1280.2,
      percent_remaining: 46.66,
      overage_permitted: false,
      reset_date: resetAt,
    },
  },
  // Org-pooled credits — no per-user quota to show.
  pooled: {
    id: 'copilot:1',
    name: 'GitHub Copilot',
    plan: 'Business',
    state: 'active',
    quotas: [],
    copilot_usage: { mode: 'ai_credits_pooled', reset_date: resetAt },
  },
};

const which = new URLSearchParams(location.search).get('state') ?? 'quota';
const service = STATES[which] ?? STATES.quota;

function Footer() {
  return (
    <footer className="mt-2 flex items-center justify-between pt-1 text-[12px] text-fg-tertiary">
      <span className="inline-flex items-center gap-1.5">
        <span aria-hidden className="text-[16px] leading-none">
          ↻
        </span>
        7s ago
      </span>
      <span className="flex gap-4">
        <span aria-hidden className="text-[17px] leading-none">
          ⚙
        </span>
        <span aria-hidden className="text-[17px] leading-none">
          ⏻
        </span>
      </span>
    </footer>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <div id="shot" className="bg-window-opaque text-fg-primary" style={{ width: 360 }}>
    <div className="px-3.5 pt-3.5 pb-2.5">
      <TokenCard service={service} warnPct={80} critPct={95} />
      <Footer />
    </div>
  </div>,
);
