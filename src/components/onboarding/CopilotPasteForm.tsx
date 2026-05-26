import { useState } from 'react';
import { setCopilotPlan, setCopilotToken, setCopilotUsername } from '../../lib/ipc';
import type { CopilotPlan } from '../../lib/ipc';
import { SegmentedControl } from '../ui/SegmentedControl';
import { SaveRow } from './SaveRow';
import type { SaveStatus } from './SaveRow';

const inputClass =
  'w-full bg-secondary border-hairline border-emphasis rounded-[6px] px-2.5 py-2 text-[12px] text-fg-primary placeholder:text-fg-quaternary outline-none focus:border-emphasis';

const PLAN_OPTIONS: { value: CopilotPlan; label: string }[] = [
  { value: 'free', label: 'Free' },
  { value: 'pro', label: 'Pro' },
  { value: 'pro_plus', label: 'Pro+' },
];

// GitHub login rules: 1–39 chars, alphanumeric or single hyphens.
const USERNAME_RE = /^[a-zA-Z0-9](?:[a-zA-Z0-9]|-(?=[a-zA-Z0-9])){0,38}$/;

export function CopilotPasteForm() {
  const [token, setToken] = useState('');
  const [username, setUsername] = useState('');
  const [plan, setPlan] = useState<CopilotPlan>('pro');
  const [status, setStatus] = useState<SaveStatus>('idle');
  const [error, setError] = useState('');

  const tokenOk = token.trim().length > 0;
  const usernameOk = USERNAME_RE.test(username.trim());
  const valid = tokenOk && usernameOk;

  async function save() {
    if (!valid) return;
    setStatus('saving');
    setError('');
    try {
      await setCopilotToken(token.trim());
      await setCopilotUsername(username.trim());
      await setCopilotPlan(plan);
      setStatus('saved');
    } catch (e) {
      setStatus('error');
      setError(String(e));
    }
  }

  return (
    <div className="flex flex-col gap-3">
      <div>
        <label htmlFor="copilot-token" className="block mb-1.5 text-[11px] text-fg-secondary">
          Personal access token
        </label>
        <input
          id="copilot-token"
          type="password"
          spellCheck={false}
          value={token}
          onChange={(e) => {
            setToken(e.target.value);
            setStatus('idle');
          }}
          placeholder="github_pat_… or ghp_…"
          className={`${inputClass} font-mono`}
        />
        <p className="mt-1.5 text-2xs text-fg-tertiary">
          Fine-grained token with Account → Plan → Read-only.
        </p>
      </div>

      <div>
        <label htmlFor="copilot-username" className="block mb-1.5 text-[11px] text-fg-secondary">
          GitHub username
        </label>
        <input
          id="copilot-username"
          spellCheck={false}
          value={username}
          onChange={(e) => {
            setUsername(e.target.value);
            setStatus('idle');
          }}
          placeholder="octocat"
          className={inputClass}
        />
        {username.trim().length > 0 && !usernameOk && (
          <p className="mt-1.5 text-2xs text-state-crit-text dark:text-state-crit-text-dark">
            Letters, numbers, and single hyphens only.
          </p>
        )}
      </div>

      <div className="flex items-center justify-between">
        <span className="text-[11px] text-fg-secondary">Plan</span>
        <SegmentedControl
          ariaLabel="Copilot plan"
          options={PLAN_OPTIONS}
          value={plan}
          onChange={(v) => {
            setPlan(v);
            setStatus('idle');
          }}
        />
      </div>

      <SaveRow status={status} error={error} disabled={!valid} onSave={save} />
    </div>
  );
}
