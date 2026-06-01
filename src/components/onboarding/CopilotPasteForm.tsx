import { useState } from 'react';
import { open as openUrl } from '@tauri-apps/plugin-shell';
import { setCopilotToken } from '../../lib/ipc';
import { TextField } from '../ui/TextField';
import { SaveRow } from './SaveRow';
import type { SaveStatus } from './SaveRow';

export function CopilotPasteForm() {
  const [token, setToken] = useState('');
  const [status, setStatus] = useState<SaveStatus>('idle');
  const [error, setError] = useState('');

  const valid = token.trim().length > 0;

  async function save() {
    if (!valid) return;
    setStatus('saving');
    setError('');
    try {
      await setCopilotToken(token.trim());
      setStatus('saved');
    } catch (e) {
      setStatus('error');
      setError(String(e));
    }
  }

  return (
    <div className="flex flex-col gap-3">
      <TextField
        label="GitHub token"
        password
        mono
        value={token}
        onChange={(v) => {
          setToken(v);
          setStatus('idle');
        }}
        placeholder="ghp_… or github_pat_…"
        hint={
          <>
            Any GitHub personal access token — no special permission needed. Your plan and quota are
            read from your account.{' '}
            <button
              type="button"
              onClick={() => openUrl('https://github.com/settings/tokens/new').catch(console.error)}
              className="underline hover:text-fg-secondary"
            >
              Create one on GitHub →
            </button>
          </>
        }
      />

      <SaveRow status={status} error={error} disabled={!valid} onSave={save} />
    </div>
  );
}
