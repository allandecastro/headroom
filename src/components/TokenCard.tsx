import { useState } from 'react';
import type { ReactNode } from 'react';
import type { Pace, ServiceStatus, Quota, CopilotUsage } from '../lib/api';
import { copilotDiagnostics } from '../lib/ipc';
import { ClaudeIcon, GitHubIcon } from './onboarding/icons';

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
  if (id === 'copilot') return <GitHubIcon />;
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
    const summary =
      service.state === 'auth_required'
        ? 'Sign in again'
        : `Couldn't fetch usage${shortHttpStatus(service.error_detail) ? ` · ${shortHttpStatus(service.error_detail)}` : ''}`;
    return (
      <section className="py-2">
        {header}
        <div
          className="text-2xs italic text-fg-quaternary"
          title={service.error_detail ?? undefined}
        >
          {summary}
        </div>
        <div className="mt-0.5 text-2xs text-fg-quaternary">
          Try “Set up accounts…” → Re-auth, or paste a token under Advanced.
        </div>
      </section>
    );
  }

  const visibleQuotas = service.quotas.filter(
    (q) => q.window !== 'claude_design' || showClaudeDesign,
  );

  return (
    <section className="py-2">
      {header}
      {visibleQuotas.length > 0 ? (
        visibleQuotas.map((q) => (
          <QuotaRow key={q.window} quota={q} warnPct={warnPct} critPct={critPct} />
        ))
      ) : (
        // Active but no numeric quota row — never render a blank card. Copilot
        // carries a regime-tagged reason (unlimited / unparsed); others fall
        // back to a neutral line.
        <EmptyUsage usage={service.copilot_usage} />
      )}
    </section>
  );
}

// Shown when a service is active but has no metered quota row to draw — an
// unlimited plan, or a payload shape Headroom couldn't classify (e.g. the new
// AI-Credits object under an id we don't recognize yet). Never a silent blank.
function EmptyUsage({ usage }: { usage?: CopilotUsage }) {
  const [copyState, setCopyState] = useState<'idle' | 'copying' | 'done' | 'error'>('idle');

  async function copyDiagnostics() {
    setCopyState('copying');
    try {
      await navigator.clipboard.writeText(await copilotDiagnostics());
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

  if (usage?.mode === 'unlimited') {
    return (
      <div className="text-2xs text-fg-quaternary">
        {usage.label} — unlimited, no metered cap on this plan
      </div>
    );
  }
  if (usage?.mode === 'unknown') {
    return (
      <div className="text-2xs text-fg-quaternary" title={usage.raw_snapshot_ids.join(', ')}>
        Couldn’t read usage — GitHub may have changed the billing format.{' '}
        <button onClick={copyDiagnostics} className="underline hover:text-fg-secondary">
          {copyLabel}
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
          {quota.unit === 'percent' ? (
            <span className={`font-medium ${state !== 'ok' ? stateClasses.text : ''}`}>{pct}%</span>
          ) : (
            <>
              <span className={`font-medium ${state !== 'ok' ? stateClasses.text : ''}`}>
                {formatNumber(quota.used, quota.unit)}
              </span>
              <span className="text-fg-quaternary">/{formatNumber(quota.total, quota.unit)}</span>
              {' · '}
              <span className={state !== 'ok' ? stateClasses.text : ''}>{pct}%</span>
            </>
          )}
        </span>
      </div>
      <div className="h-0.5 my-1 bg-black/10 dark:bg-white/10 overflow-hidden">
        <div className={`h-full ${stateClasses.bar}`} style={{ width: `${Math.min(100, pct)}%` }} />
      </div>
      <div className="text-2xs text-fg-tertiary tabular-nums">
        {formatResetTime(quota.resets_at)}
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
function shortHttpStatus(detail?: string): string | null {
  if (!detail) return null;
  const m = detail.match(/^HTTP\s+(\d{3})/i);
  return m ? `HTTP ${m[1]}` : null;
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
