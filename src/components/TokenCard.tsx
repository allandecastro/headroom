import type { ServiceStatus, Quota } from '../lib/api';

interface Props {
  service: ServiceStatus;
}

export function TokenCard({ service }: Props) {
  if (service.state === 'unreachable') {
    return (
      <section className="py-2 border-t border-hairline border-default first:border-t-0">
        <header className="flex items-center gap-2 mb-2.5 text-[11px] font-medium uppercase tracking-[0.08em] text-fg-tertiary">
          <span className="opacity-50">{service.name}</span>
          <span className="ml-auto text-state-warn-text-dark text-[10px] inline-flex items-center gap-1">
            ⚠ unreachable
          </span>
        </header>
        <div className="text-2xs text-fg-quaternary italic">
          {service.error_detail ?? 'No data available'}
        </div>
      </section>
    );
  }

  return (
    <section className="py-2 border-t border-hairline border-default first:border-t-0">
      <header className="flex items-center gap-2 mb-2.5 text-[11px] font-medium uppercase tracking-[0.08em] text-fg-tertiary">
        <span>{service.name}</span>
        <span className="ml-auto text-[10px] font-normal normal-case tracking-normal text-fg-quaternary">
          {service.plan}
        </span>
      </header>

      {service.quotas.map((q) => (
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
          <span className={`font-medium ${state !== 'ok' ? stateClasses.text : ''}`}>
            {formatNumber(quota.used, quota.unit)}
          </span>
          <span className="text-fg-quaternary">/{formatNumber(quota.total, quota.unit)}</span>
          {' · '}
          <span className={state !== 'ok' ? stateClasses.text : ''}>{pct}%</span>
        </span>
      </div>
      <div className="h-0.5 my-1 bg-black/10 dark:bg-white/10 overflow-hidden">
        <div className={`h-full ${stateClasses.bar}`} style={{ width: `${Math.min(100, pct)}%` }} />
      </div>
      <div className="text-2xs text-fg-tertiary tabular-nums">
        {formatResetTime(quota.resets_at)}
      </div>
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
