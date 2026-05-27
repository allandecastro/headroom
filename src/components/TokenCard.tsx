import type { ReactNode } from 'react';
import type { ServiceStatus, Quota } from '../lib/api';
import { ClaudeIcon, GitHubIcon } from './onboarding/icons';

interface Props {
  service: ServiceStatus;
  showClaudeDesign?: boolean;
}

function serviceIcon(id: string): ReactNode {
  if (id === 'claude') return <ClaudeIcon />;
  if (id === 'copilot') return <GitHubIcon />;
  return null;
}

export function TokenCard({ service, showClaudeDesign = false }: Props) {
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
    return (
      <section className="py-2">
        {header}
        <div className="text-2xs italic text-fg-quaternary">
          {service.state === 'auth_required'
            ? 'Sign in again'
            : (service.error_detail ?? 'Unreachable')}
        </div>
      </section>
    );
  }

  return (
    <section className="py-2">
      {header}
      {service.quotas
        .filter((q) => q.window !== 'claude_design' || showClaudeDesign)
        .map((q) => (
          <QuotaRow key={q.window} quota={q} />
        ))}
    </section>
  );
}

function QuotaRow({ quota }: { quota: Quota }) {
  const pct = Math.round((quota.used / quota.total) * 100);
  const state = pct >= 95 ? 'crit' : pct >= 80 ? 'warn' : 'ok';

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
        <div className="mt-1 flex items-center gap-2">
          {(quota.sparkline?.length ?? 0) >= 2 && (
            <span className={state === 'ok' ? 'text-fg-tertiary' : stateClasses.text}>
              <Sparkline points={quota.sparkline!} />
            </span>
          )}
          {quota.projection && (
            <span
              className={`text-2xs ${
                quota.projection.will_exceed ? 'text-state-warn-text-dark' : 'text-fg-tertiary'
              }`}
            >
              {quota.projection.will_exceed
                ? `On track to exceed · ~${Math.round(quota.projection.projected_pct)}% by reset${
                    quota.projection.eta ? ` · full ${relativeFromNow(quota.projection.eta)}` : ''
                  }`
                : `On track · ~${Math.round(quota.projection.projected_pct)}% by reset`}
            </span>
          )}
        </div>
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

// Tiny inline sparkline of recent utilization (0–100%), inheriting currentColor.
function Sparkline({ points }: { points: number[] }) {
  const W = 48;
  const H = 12;
  const n = points.length;
  const coords = points
    .map((p, i) => {
      const x = (i / (n - 1)) * W;
      const y = H - (Math.min(100, Math.max(0, p)) / 100) * H;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(' ');
  return (
    <svg width={W} height={H} viewBox={`0 0 ${W} ${H}`} aria-hidden className="block">
      <polyline
        points={coords}
        fill="none"
        stroke="currentColor"
        strokeWidth="1"
        strokeLinejoin="round"
        strokeLinecap="round"
      />
    </svg>
  );
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
