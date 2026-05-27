import { useState } from 'react';
import { setCopilotPlan, setCopilotToken, setCopilotUsername } from '../../lib/ipc';
import type { CopilotPlan } from '../../lib/ipc';
import { SegmentedControl } from '../ui/SegmentedControl';
import { TextField } from '../ui/TextField';
import { SaveRow } from './SaveRow';
import type { SaveStatus } from './SaveRow';

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
      <TextField
        label="Personal access token"
        password
        mono
        value={token}
        onChange={(v) => {
          setToken(v);
          setStatus('idle');
        }}
        placeholder="github_pat_… or ghp_…"
        hint="Fine-grained token with Account → Plan → Read-only."
      />

      <TextField
        label="GitHub username"
        value={username}
        onChange={(v) => {
          setUsername(v);
          setStatus('idle');
        }}
        placeholder="octocat"
        error={
          username.trim().length > 0 && !usernameOk
            ? 'Letters, numbers, and single hyphens only.'
            : undefined
        }
      />

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
