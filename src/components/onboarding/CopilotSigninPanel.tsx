import type { DeviceCode, CopilotPlan } from '../../lib/ipc';
import { SegmentedControl } from '../ui/SegmentedControl';

export type SigninStatus = 'pending' | 'success' | 'error';

interface Props {
  device: DeviceCode;
  status: SigninStatus;
  error?: string;
  plan: CopilotPlan;
  onPlanChange: (plan: CopilotPlan) => void;
  onOpen: () => void;
}

const PLAN_OPTIONS: { value: CopilotPlan; label: string }[] = [
  { value: 'free', label: 'Free' },
  { value: 'pro', label: 'Pro' },
  { value: 'pro_plus', label: 'Pro+' },
];

// Shown directly under the Copilot "Sign in with GitHub" button while the
// device flow is in progress (and after).
export function CopilotSigninPanel({ device, status, error, plan, onPlanChange, onOpen }: Props) {
  return (
    <div className="my-2 rounded-[6px] border-hairline border-emphasis bg-black/[0.03] px-3 py-2.5 text-[12px] dark:bg-white/[0.04]">
      {status === 'pending' && (
        <>
          <div className="mb-1.5 text-fg-secondary">Enter this code on GitHub:</div>
          <div className="mb-2 flex items-center justify-between gap-2">
            <span className="font-mono text-[18px] tracking-[0.15em] text-fg-primary">
              {device.user_code}
            </span>
            <button
              type="button"
              onClick={onOpen}
              className="text-[11px] text-fg-secondary underline hover:text-fg-primary"
            >
              Open GitHub
            </button>
          </div>
          <div className="text-[10.5px] text-fg-tertiary">Waiting for authorization…</div>
        </>
      )}

      {status === 'success' && (
        <>
          <div className="mb-2 text-state-ok-text-dark">Signed in to GitHub</div>
          <div className="flex items-center justify-between gap-2">
            <span className="text-[11px] text-fg-secondary">Your plan</span>
            <SegmentedControl
              ariaLabel="Copilot plan"
              options={PLAN_OPTIONS}
              value={plan}
              onChange={onPlanChange}
            />
          </div>
        </>
      )}

      {status === 'error' && (
        <div className="text-state-crit-text-dark">{error ?? 'Sign-in failed'}</div>
      )}
    </div>
  );
}
