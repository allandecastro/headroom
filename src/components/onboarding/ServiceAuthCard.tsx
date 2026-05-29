import { useState } from 'react';
import type { ReactNode } from 'react';
import { ExternalLinkIcon, KeyIcon } from './icons';

interface Props {
  icon: ReactNode;
  name: string;
  /** When present, renders the magic sign-in button + an "Advanced" disclosure
   * around `children`. When absent, the card is paste-only: `children` is shown
   * directly with no magic button or separator. */
  primaryLabel?: string;
  advancedLabel?: string;
  children: ReactNode; // the paste form
  onPrimary?: () => void;
  extra?: ReactNode; // optional status content rendered directly under the primary button
}

// One service block in the onboarding picker.
// See docs/mockups/02-onboarding.html (.svc-card).
export function ServiceAuthCard({
  icon,
  name,
  primaryLabel,
  advancedLabel,
  children,
  onPrimary,
  extra,
}: Props) {
  const [expanded, setExpanded] = useState(false);

  const header = (
    <div className="flex items-center gap-2 mb-3 text-[11px] font-medium uppercase tracking-[0.08em] text-fg-tertiary">
      <span className="inline-flex text-fg-secondary">{icon}</span>
      {name}
    </div>
  );

  // Paste-only mode: just the header + the form, no magic button.
  if (!onPrimary || !primaryLabel) {
    return (
      <div className="border-hairline border-default rounded-[8px] px-3.5 pt-3.5 pb-3 mb-2.5">
        {header}
        {children}
      </div>
    );
  }

  return (
    <div className="border-hairline border-default rounded-[8px] px-3.5 pt-3.5 pb-3 mb-2.5">
      {header}

      <button
        type="button"
        onClick={onPrimary}
        className="w-full flex items-center justify-center gap-[7px] mb-2 px-3.5 py-[9px] rounded-[6px] border-hairline border-emphasis bg-black/[0.05] dark:bg-white/[0.08] text-[12.5px] font-medium text-fg-primary hover:bg-black/[0.08] dark:hover:bg-white/[0.12]"
      >
        <ExternalLinkIcon />
        {primaryLabel}
      </button>

      {extra}

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
