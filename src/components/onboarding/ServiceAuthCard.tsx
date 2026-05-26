import { useState } from 'react';
import type { ReactNode } from 'react';
import { ExternalLinkIcon, KeyIcon } from './icons';

interface Props {
  icon: ReactNode;
  name: string;
  primaryLabel: string;
  advancedLabel: string;
  children: ReactNode; // the paste form, revealed when "advanced" is expanded
}

// One service block in the onboarding picker.
// See docs/mockups/02-onboarding.html (.svc-card). The primary "magic" sign-in
// button is a Phase 2 stub (disabled); the paste path under "advanced" is live.
export function ServiceAuthCard({ icon, name, primaryLabel, advancedLabel, children }: Props) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="border-hairline border-default rounded-[8px] px-3.5 pt-3.5 pb-3 mb-2.5">
      <div className="flex items-center gap-2 mb-3 text-[11px] font-medium uppercase tracking-[0.08em] text-fg-tertiary">
        <span className="inline-flex text-fg-secondary">{icon}</span>
        {name}
      </div>

      <button
        type="button"
        disabled
        title="Direct sign-in is coming in a later version — use “advanced” below to paste a token for now"
        className="w-full flex items-center justify-center gap-[7px] mb-2 px-3.5 py-[9px] rounded-[6px] border-hairline border-emphasis bg-black/[0.05] dark:bg-white/[0.08] text-[12.5px] font-medium text-fg-primary opacity-40 cursor-not-allowed"
      >
        <ExternalLinkIcon />
        {primaryLabel}
        <span className="ml-1 text-[9px] uppercase tracking-[0.08em] text-fg-tertiary">soon</span>
      </button>

      <div className="flex items-center gap-2 my-2 text-[10px] uppercase tracking-[0.08em] text-fg-quaternary before:flex-1 before:h-[0.5px] before:bg-[var(--border-default)] before:content-[''] after:flex-1 after:h-[0.5px] after:bg-[var(--border-default)] after:content-['']">
        or
      </div>

      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        aria-expanded={expanded}
        className="w-full flex items-center justify-between px-[11px] py-[7px] rounded-[6px] border-hairline border-dashed border-emphasis text-[11.5px] text-fg-secondary hover:bg-secondary"
      >
        <span className="inline-flex items-center gap-1.5">
          <span className="inline-flex text-fg-tertiary">
            <KeyIcon />
          </span>
          {advancedLabel}
        </span>
        <span className="text-[11px] text-fg-tertiary">advanced {expanded ? '⌄' : '›'}</span>
      </button>

      {expanded && <div className="mt-3">{children}</div>}
    </div>
  );
}
