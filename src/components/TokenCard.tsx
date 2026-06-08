import { useState } from 'react';
import type { ReactNode } from 'react';
import type {
  Pace,
  ServiceStatus,
  Quota,
  CopilotUsage,
  CodexMeta,
  CodexTokenStats,
} from '../lib/api';
import { copilotDiagnostics, codexDiagnostics } from '../lib/ipc';
import { ClaudeIcon, GitHubIcon, CodexIcon } from './onboarding/icons';

interface Props {
  service: ServiceStatus;
  showClaudeDesign?: boolean;
  /** Percentage at which a quota turns amber (0 = off). */
  warnPct: number;
  /** Percentage at which a quota turns red (0 = off). */
  critPct: number;
}

function serviceIcon(id: string): ReactNode {
  if (id === 'claude') return <ClaudeIcon />;
  if (id === 'codex') return <CodexIcon />;
  // Copilot ids are "copilot" (legacy) or "copilot:<account-id>" (multi-account).
  if (id === 'copilot' || id.startsWith('copilot:')) return <GitHubIcon />;
  return null;
}

export function TokenCard({ service, showClaudeDesign = false, warnPct, critPct }: Props) {
  const header = (
    <header className="mb-2.5 flex items-center gap-2 text-[11px] font-medium uppercase tracking-[0.08em] text-fg-tertiary">
      <span className="inline-flex text-fg-secondary">{serviceIcon(service.id)}</span>
      <span>{service.name}</span>
      {service.state === 'active' && service.plan && (
        <span className="ml-auto text-[10px] font-normal normal-case tracking-normal text-fg-quaternary">
          {service.plan}
        </span>
      )}
    </header>
  );

  if (service.state === 'needs_setup') {
    return (
      <section className="py-2">
        {header}
        <div className="text-2xs text-fg-quaternary">Not connected — open “Set up accounts…”</div>
      </section>
    );
  }

  if (service.state === 'unreachable' || service.state === 'auth_required') {
    const { summary, hint } = errorCopy(service.state, service.error_detail);
    return (
      <section className="py-2">
        {header}
        <div
          className="text-2xs italic text-fg-quaternary"
          title={service.error_detail ?? undefined}
        >
          {summary}
        </div>
        {hint && <div className="mt-0.5 text-2xs text-fg-quaternary">{hint}</div>}
      </section>
    );
  }

  const visibleQuotas = service.quotas.filter(
    (q) => q.window !== 'claude_design' || showClaudeDesign,
  );
  const isCodex = service.id === 'codex';

  return (
    <section className="py-2">
      {header}
      {visibleQuotas.length > 0 ? (
        <>
          {visibleQuotas.map((q) => (
            <QuotaRow key={q.window} quota={q} warnPct={warnPct} critPct={critPct} />
          ))}
          {isCodex && service.codex_meta && <CodexExtras meta={service.codex_meta} />}
        </>
      ) : isCodex ? (
        // Codex installed but no rate-limit numbers yet (e.g. only exec-mode
        // sessions, or not signed into ChatGPT). Show the guidance + any token
        // stats we did read, plus a diagnostics copy.
        <CodexEmpty meta={service.codex_meta} />
      ) : (
        // Active but no numeric quota row — never render a blank card. Copilot
        // carries a regime-tagged reason (unlimited / unparsed); others fall
        // back to a neutral line.
        <EmptyUsage
          usage={service.copilot_usage}
          accountId={
            service.id.startsWith('copilot:') ? service.id.slice('copilot:'.length) : undefined
          }
        />
      )}
    </section>
  );
}

// The "as of / source" line, credits, and token-consumption stats shown under
// Codex's % rows. Codex data only refreshes when you run Codex (logs) or when the
// live endpoint is reachable, so surfacing freshness + source matters.
function CodexExtras({ meta }: { meta: CodexMeta }) {
  const sourceLabel = meta.source === 'live' ? 'live' : 'local logs';
  return (
    <div className="mt-1 space-y-0.5">
      <div className="text-2xs text-fg-quaternary">
        {meta.captured_at ? `Updated ${formatAgo(meta.captured_at)}` : 'Updated'} · {sourceLabel}
        {meta.stale && ' · may be stale'}
        {meta.token_expired && ' · sign-in expired'}
      </div>
      {meta.credits_balance && (
        <div className="text-2xs text-fg-tertiary tabular-nums">
          Credits: {meta.credits_balance}
        </div>
      )}
      {meta.token_stats && <CodexTokenStatsRow stats={meta.token_stats} />}
    </div>
  );
}

function CodexTokenStatsRow({ stats }: { stats: CodexTokenStats }) {
  return (
    <div
      className="text-2xs text-fg-quaternary tabular-nums"
      title={`input ${stats.input.toLocaleString('en-US')} · cached ${stats.cached_input.toLocaleString(
        'en-US',
      )} · output ${stats.output.toLocaleString('en-US')} · reasoning ${stats.reasoning.toLocaleString(
        'en-US',
      )}`}
    >
      {formatTokens(stats.total)} tokens · {stats.window_label}
    </div>
  );
}

// Codex active but with no % rows to draw — guidance + any token stats + a
// diagnostics copy. Mirrors EmptyUsage's copy-state machinery.
function CodexEmpty({ meta }: { meta?: CodexMeta }) {
  const [copyState, setCopyState] = useState<'idle' | 'copying' | 'done' | 'error'>('idle');

  async function copyDiagnostics() {
    setCopyState('copying');
    try {
      await navigator.clipboard.writeText(await codexDiagnostics());
      setCopyState('done');
    } catch (e) {
      console.error(e);
      setCopyState('error');
    }
  }

  const copyLabel =
    copyState === 'copying'
      ? 'Copying…'
      : copyState === 'done'
        ? 'Copied ✓'
        : copyState === 'error'
          ? 'Copy failed'
          : 'Copy Codex diagnostics';

  return (
    <div>
      <div className="text-2xs text-fg-tertiary">{meta?.note ?? 'No usage data yet.'}</div>
      {meta?.token_stats && (
        <div className="mt-0.5">
          <CodexTokenStatsRow stats={meta.token_stats} />
        </div>
      )}
      <button
        onClick={copyDiagnostics}
        className="mt-1 text-2xs text-fg-quaternary underline hover:text-fg-secondary"
      >
        {copyLabel}
      </button>
    </div>
  );
}

// "1800" → "1.8K", "2400000" → "2.4M". Token totals get large; keep it compact.
function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return `${n}`;
}

// "5m ago" / "2h 13m ago" / "3d 4h ago" — past-relative counterpart to
// relativeFromNow, for Codex's "Updated …" line.
function formatAgo(iso: string): string {
  const diffMs = Date.now() - new Date(iso).getTime();
  if (diffMs < 60_000) return 'just now';
  const totalMin = Math.floor(diffMs / 60_000);
  const days = Math.floor(totalMin / 1440);
  const hours = Math.floor((totalMin % 1440) / 60);
  const mins = totalMin % 60;
  if (days >= 1) return `${days}d ${hours}h ago`;
  if (hours >= 1) return `${hours}h ${mins}m ago`;
  return `${mins}m ago`;
}

// Shown when a service is active but has no metered quota row to draw — an
// unlimited plan, or a payload shape Headroom couldn't classify (e.g. the new
// AI-Credits object under an id we don't recognize yet). Never a silent blank.
function EmptyUsage({ usage, accountId }: { usage?: CopilotUsage; accountId?: string }) {
  const [copyState, setCopyState] = useState<'idle' | 'copying' | 'done' | 'error'>('idle');

  async function copyDiagnostics() {
    setCopyState('copying');
    try {
      await navigator.clipboard.writeText(await copilotDiagnostics(accountId));
      setCopyState('done');
    } catch (e) {
      console.error(e);
      setCopyState('error');
    }
  }

  const copyLabel =
    copyState === 'copying'
      ? 'Copying…'
      : copyState === 'done'
        ? 'Copied ✓'
        : copyState === 'error'
          ? 'Copy failed'
          : 'Copy diagnostics';

  if (usage?.mode === 'ai_credits_pooled') {
    // Org-managed pool, no per-seat quota in the payload. Deliberately no bar,
    // no percentage, no count — those would be misleading here. The "why" + CTA
    // live in the tooltip to keep the menu-bar view short.
    return (
      <div title="Ask an admin to set a user-level budget to track your usage here.">
        <div className="text-2xs text-fg-secondary">Pooled — no individual quota</div>
        <div className="mt-0.5 text-2xs text-fg-quaternary">
          Your AI credits are shared at the org level, so there’s no per-user usage to show.
        </div>
      </div>
    );
  }
  if (usage?.mode === 'unknown') {
    return (
      <div title={usage.raw_snapshot_ids.join(', ')}>
        <div className="text-2xs text-fg-secondary">Couldn’t read your quota format</div>
        <button
          onClick={copyDiagnostics}
          className="mt-0.5 text-2xs text-fg-quaternary underline hover:text-fg-secondary"
        >
          {copyLabel === 'Copy diagnostics' ? 'Copy raw payload' : copyLabel}
        </button>
      </div>
    );
  }
  return <div className="text-2xs text-fg-quaternary">No usage data.</div>;
}

function QuotaRow({ quota, warnPct, critPct }: { quota: Quota; warnPct: number; critPct: number }) {
  const [expanded, setExpanded] = useState(false);
  const pct = Math.round((quota.used / quota.total) * 100);
  // A threshold of 0 is "off" — that level never colours the row, matching the
  // tray's compute_state in src-tauri/src/tray.rs.
  const state =
    critPct > 0 && pct >= critPct ? 'crit' : warnPct > 0 && pct >= warnPct ? 'warn' : 'ok';

  const stateClasses = {
    ok: { bar: 'bg-state-ok-fill-dark', text: 'text-state-ok-text-dark' },
    warn: { bar: 'bg-state-warn-fill-dark', text: 'text-state-warn-text-dark' },
    crit: { bar: 'bg-state-crit-fill-dark', text: 'text-state-crit-text-dark' },
  }[state];

  return (
    <div className="mb-2 last:mb-0">
      <div className="flex items-baseline justify-between text-[11px]">
        <span className="text-fg-secondary">{quota.label}</span>
        <span className="font-mono text-xxs tabular-nums">
          <span className={`font-medium ${state !== 'ok' ? stateClasses.text : ''}`}>{pct}%</span>
        </span>
      </div>
      <div className="h-0.5 my-1 bg-black/10 dark:bg-white/10 overflow-hidden">
        <div className={`h-full ${stateClasses.bar}`} style={{ width: `${Math.min(100, pct)}%` }} />
      </div>
      {/* Subline: raw remaining/entitlement (no unit word — it differs by quota
          and isn't reliably known) + reset. Percent quotas have no entitlement,
          so they keep just the reset. */}
      <div className="text-2xs text-fg-tertiary tabular-nums">
        {quota.unit === 'percent'
          ? formatResetTime(quota.resets_at)
          : `${formatNumber(quota.total - quota.used, quota.unit)} / ${formatNumber(
              quota.total,
              quota.unit,
            )} left · resets ${formatResetTime(quota.resets_at)}`}
      </div>
      {(quota.projection || (quota.sparkline?.length ?? 0) >= 2) && (
        <>
          <button
            type="button"
            onClick={() => setExpanded((v) => !v)}
            aria-expanded={expanded}
            aria-label={expanded ? 'Hide trend chart' : 'Show trend chart'}
            className="mt-1 flex w-full items-center gap-2 text-left hover:opacity-80"
          >
            {quota.projection ? (
              <span
                className={`flex-1 text-2xs ${
                  quota.projection.will_exceed ? 'text-state-warn-text-dark' : 'text-fg-tertiary'
                }`}
              >
                {quota.projection.will_exceed
                  ? `On track to exceed · ~${Math.round(quota.projection.projected_pct)}% by reset${
                      quota.projection.eta ? ` · full ${relativeFromNow(quota.projection.eta)}` : ''
                    }`
                  : `On track · ~${Math.round(quota.projection.projected_pct)}% by reset`}
              </span>
            ) : (
              <span className="flex-1 text-2xs text-fg-tertiary">Recent trend</span>
            )}
            <span className="text-[10px] text-fg-quaternary" aria-hidden>
              {expanded ? '▴' : '▾'}
            </span>
          </button>
          {expanded &&
            ((quota.sparkline?.length ?? 0) >= 2 ? (
              <ExpandedChart
                points={quota.sparkline!}
                pace={quota.pace}
                colorClass={state === 'ok' ? 'text-fg-tertiary' : stateClasses.text}
              />
            ) : (
              <div className="mt-1.5 rounded-[6px] bg-black/[0.03] px-3 py-2 text-2xs italic text-fg-quaternary dark:bg-white/[0.04]">
                collecting trend… one sample every ~5 minutes
              </div>
            ))}
        </>
      )}
      {state === 'crit' && quota.advice && (
        <div className={`text-2xs mt-0.5 ${stateClasses.text}`}>{quota.advice}</div>
      )}
    </div>
  );
}

function formatNumber(value: number, unit: string): string {
  if (unit === 'hours') return `${Math.round(value)}h`;
  if (unit === 'usd_credits') return `$${value.toFixed(2)}`;
  return value.toLocaleString('en-US');
}

// Larger, readable trend chart shown inline when a quota row is expanded.
// Stretches to the parent's width and shows min / now / max under the line,
// plus the optional recent-burn-rate pace.
function ExpandedChart({
  points,
  pace,
  colorClass,
}: {
  points: number[];
  pace?: Pace;
  colorClass: string;
}) {
  const W = 300;
  const H = 60;
  const n = points.length;
  const coords = points
    .map((p, i) => {
      const x = (i / (n - 1)) * W;
      const y = H - (Math.min(100, Math.max(0, p)) / 100) * H;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(' ');
  // ExpandedChart is only rendered when `points.length >= 2`, so `current` is
  // guaranteed defined — the assertion silences noUncheckedIndexedAccess.
  const current = points[points.length - 1]!;
  const max = Math.max(...points);
  const min = Math.min(...points);

  return (
    <div className="mt-1.5 rounded-[6px] bg-black/[0.03] px-2 py-2 dark:bg-white/[0.04]">
      {pace && (
        <div
          className={`mb-1.5 text-[10.5px] tabular-nums ${
            pace.over_pace && !pace.low_confidence
              ? 'text-state-warn-text-dark'
              : 'text-fg-tertiary'
          }`}
        >
          Burning {pace.low_confidence ? '~' : ''}
          {Math.round(pace.daily_rate)}%/day
          {pace.over_pace
            ? ` — over the ${Math.round(pace.safe_pace)}%/day safe pace`
            : ` · ${Math.round(pace.safe_pace)}%/day keeps you on track`}
          {pace.low_confidence && ' · rough (history gap)'}
        </div>
      )}
      <div className={colorClass}>
        <svg
          width="100%"
          height={H}
          viewBox={`0 0 ${W} ${H}`}
          preserveAspectRatio="none"
          aria-hidden
          className="block"
        >
          {/* 100% reference line at the top of the area. */}
          <line
            x1="0"
            y1="0.5"
            x2={W}
            y2="0.5"
            stroke="currentColor"
            strokeWidth="0.5"
            strokeOpacity="0.25"
            strokeDasharray="3 3"
          />
          <polyline
            points={coords}
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinejoin="round"
            strokeLinecap="round"
          />
        </svg>
      </div>
      <div className="mt-1 flex justify-between text-[10px] tabular-nums text-fg-quaternary">
        <span>min {Math.round(min)}%</span>
        <span>now {Math.round(current)}%</span>
        <span>max {Math.round(max)}%</span>
      </div>
    </div>
  );
}

// "HTTP 404: {body}" → "HTTP 404"; keeps the long body for the tooltip.
// Maps a failed service to a distinct, plain-language status line (+ optional
// one-line hint). Never empty — every failure mode has its own message, so the
// card never renders blank or just spins. The raw error_detail goes in a tooltip.
function errorCopy(
  state: 'unreachable' | 'auth_required',
  detail?: string,
): { summary: string; hint?: string } {
  if (state === 'auth_required') {
    return {
      summary: 'Token expired or invalid — re-authenticate',
      hint: 'Open “Set up accounts…” → Re-auth, or paste a token under Advanced.',
    };
  }
  const status = detail?.match(/^HTTP\s+(\d{3})/i)?.[1];
  if (status === '429') return { summary: 'Rate limited by GitHub — try again shortly' };
  if (status) return { summary: `GitHub returned an error (${status})` };
  // Network error, timeout, Cloudflare challenge, unparseable response.
  return {
    summary: "Couldn't reach GitHub — check your connection",
    hint: 'Headroom will retry on the next refresh.',
  };
}

function relativeFromNow(iso: string): string {
  const diffMs = new Date(iso).getTime() - Date.now();
  if (diffMs <= 0) return 'now';
  const totalMin = Math.floor(diffMs / 60_000);
  const days = Math.floor(totalMin / 1440);
  const hours = Math.floor((totalMin % 1440) / 60);
  const mins = totalMin % 60;
  if (days >= 1) return `in ${days}d ${hours}h`;
  if (hours >= 1) return `in ${hours}h ${mins}m`;
  return `in ${mins}m`;
}

function formatResetTime(resetsAt: string): string {
  const target = new Date(resetsAt);
  const diffMs = target.getTime() - Date.now();
  if (diffMs <= 0) return 'resetting…';
  const totalMin = Math.floor(diffMs / 60_000);
  const days = Math.floor(totalMin / 1440);
  const hours = Math.floor((totalMin % 1440) / 60);
  const mins = totalMin % 60;

  const localTime = target.toLocaleString('en-US', {
    weekday: days >= 1 ? 'short' : undefined,
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  });

  if (days >= 1) return `${localTime} · in ${days}d ${hours}h`;
  if (hours >= 1) return `${localTime} · in ${hours}h ${mins}m`;
  return `${localTime} · in ${mins}m`;
}
