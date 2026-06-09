// Screenshot harness — renders the real Headroom UI with canned data so the
// capture script (capture.mjs) can produce deterministic README screenshots
// without the Tauri backend. Two kinds of state:
//   - card states  → a single popover card (TokenCard), wrapped in #shot
//   - window states (popover / settings / onboarding) → the real full window
//     component, with Tauri IPC mocked
// Not part of the app build (index.html is the only bundled entry).
import ReactDOM from 'react-dom/client';
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import { TokenCard } from '../../src/components/TokenCard';
import App from '../../src/App';
import SettingsPanel from '../../src/SettingsPanel';
import OnboardingFlow from '../../src/OnboardingFlow';
import type { ServiceStatus, Snapshot } from '../../src/lib/api';
import type { Settings, CopilotAccount } from '../../src/lib/ipc';
import '../../src/index.css';

const H = 3_600_000;
const iso = (ms: number) => new Date(Date.now() + ms).toISOString();
const resetAt = iso((27 * 24 + 15) * H);

// ── Mock data ────────────────────────────────────────────────────────────────

const pct = (window: ServiceStatus['quotas'][number]['window'], label: string, p: number) => ({
  window,
  label,
  used: p,
  total: 100,
  unit: 'percent' as const,
  resets_at: resetAt,
});

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

const CLAUDE: ServiceStatus = {
  id: 'claude',
  name: 'Claude',
  plan: '',
  state: 'active',
  quotas: [
    pct('five_hour', 'Current session', 18),
    pct('weekly_all', 'Weekly · 7d', 23),
    pct('weekly_sonnet', 'Sonnet · 7d', 6),
  ],
  copilot_usage: undefined,
};

// Codex reads its windows + token stats locally (live `/wham/usage` or the
// rollout logs) — see src-tauri/src/sources/codex.rs.
const CODEX: ServiceStatus = {
  id: 'codex',
  name: 'Codex',
  plan: 'Pro',
  state: 'active',
  quotas: [pct('five_hour', 'Current session · 5h', 31), pct('weekly_all', 'Weekly · 7d', 12)],
  copilot_usage: undefined,
  codex_meta: {
    source: 'live',
    captured_at: iso(-3 * 60_000), // 3 minutes ago
    credits_balance: '$8.50',
    token_stats: {
      input: 720_000,
      cached_input: 410_000,
      output: 380_000,
      reasoning: 140_000,
      total: 1_240_000,
      window_label: 'last 24h',
    },
  },
};

const ACCOUNTS: CopilotAccount[] = [
  { id: '1', login: 'octocat', label: 'octocat' },
  { id: '2', login: 'acme-bot', label: 'Acme Corp' },
];

const SNAPSHOT: Snapshot = {
  polled_at: Math.floor(Date.now() / 1000) - 7,
  services: [
    CLAUDE,
    CODEX,
    copilotAccount('copilot:1', 'octocat', 53),
    copilotAccount('copilot:2', 'Acme Corp', 88),
  ],
};

const SETTINGS: Settings = {
  poll_interval_secs: 30,
  theme: 'auto',
  show_tray_percentage: true,
  notify_warn_pct: 80,
  notify_crit_pct: 95,
  show_claude_design: false,
  check_updates: true,
  codex_live_query: true,
  notified_update_version: '',
};

// Mock the Tauri IPC the window components call on mount. Anything unlisted
// (window sizing, event subscriptions) returns null — useFitWindow swallows it.
mockWindows('main');
mockIPC(
  (cmd) => {
    switch (cmd) {
      case 'refresh_all':
        return SNAPSHOT;
      case 'get_settings':
        return SETTINGS;
      case 'get_autostart':
        return false;
      case 'get_update':
        return null;
      case 'list_copilot_accounts':
        return ACCOUNTS;
      case 'plugin:app|version':
        return '1.5.0';
      default:
        return null;
    }
  },
  { shouldMockEvents: true },
);

// ── Card states (single popover card) ────────────────────────────────────────

const CARD_STATES: Record<string, ServiceStatus[]> = {
  quota: [copilotAccount('copilot:1', 'GitHub Copilot', 53)],
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
};

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

function Cards({ services }: { services: ServiceStatus[] }) {
  return (
    <div id="shot" className="bg-window-opaque text-fg-primary" style={{ width: 360 }}>
      <div className="px-3.5 pt-3.5 pb-2.5">
        {services.map((service) => (
          <TokenCard key={service.id} service={service} warnPct={80} critPct={95} />
        ))}
        <Footer />
      </div>
    </div>
  );
}

// ── Render the requested state ───────────────────────────────────────────────

const which = new URLSearchParams(location.search).get('state') ?? 'quota';
const root = ReactDOM.createRoot(document.getElementById('root')!);

if (which === 'popover') root.render(<App />);
else if (which === 'settings') root.render(<SettingsPanel />);
else if (which === 'onboarding') root.render(<OnboardingFlow />);
else root.render(<Cards services={CARD_STATES[which] ?? CARD_STATES.quota} />);
