// Screenshot harness — renders the real popover card(s) with canned data so the
// capture script (capture.mjs) can produce deterministic README screenshots
// without the Tauri backend. Not part of the app build (index.html is the only
// bundled entry). Add a state to STATES, then reference it in capture.mjs.
import ReactDOM from 'react-dom/client';
import { TokenCard } from '../../src/components/TokenCard';
import type { ServiceStatus } from '../../src/lib/api';
import '../../src/index.css';

const H = 3_600_000;
const iso = (ms: number) => new Date(Date.now() + ms).toISOString();
const resetAt = iso((27 * 24 + 15) * H);

// A connected GitHub Copilot account with an AI-Credits quota at `usedPct`.
// `id` only needs to start with "copilot:" (drives the GitHub icon); `label`
// is the card header (the account's login by default, or a user rename).
function copilotAccount(id: string, label: string, usedPct: number): ServiceStatus {
  const total = 2400;
  const used = Math.round(total * usedPct) / 100;
  return {
    id,
    name: label,
    plan: 'Business',
    state: 'active',
    quotas: [
      { window: 'monthly', label: 'AI Credits', used, total, unit: 'requests', resets_at: resetAt },
    ],
    copilot_usage: {
      mode: 'ai_credits_capped',
      label: 'AI Credits',
      entitlement: total,
      remaining: total - used,
      used,
      percent_remaining: 100 - usedPct,
      overage_permitted: false,
      reset_date: resetAt,
    },
  };
}

// Each state is one or more service cards, rendered stacked like the popover.
const STATES: Record<string, ServiceStatus[]> = {
  // Business with a per-user AI-Credits quota assigned.
  quota: [copilotAccount('copilot:1', 'GitHub Copilot', 53)],
  // Org-pooled credits — no per-user quota to show.
  pooled: [
    {
      id: 'copilot:1',
      name: 'GitHub Copilot',
      plan: 'Business',
      state: 'active',
      quotas: [],
      copilot_usage: { mode: 'ai_credits_pooled', reset_date: resetAt },
    },
  ],
  // Two connected GitHub accounts — one card each, with their own usage. The
  // second is renamed to its org; the first keeps its login.
  'multi-account': [
    copilotAccount('copilot:1', 'octocat', 53),
    copilotAccount('copilot:2', 'Acme Corp', 88),
  ],
};

const which = new URLSearchParams(location.search).get('state') ?? 'quota';
const services = STATES[which] ?? STATES.quota;

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
      {services.map((service) => (
        <TokenCard key={service.id} service={service} warnPct={80} critPct={95} />
      ))}
      <Footer />
    </div>
  </div>,
);
