import { useState } from 'react';
import { setClaudeSession } from '../../lib/ipc';
import { TextField } from '../ui/TextField';
import { SaveRow } from './SaveRow';
import type { SaveStatus } from './SaveRow';

export function ClaudePasteForm() {
  const [value, setValue] = useState('');
  const [status, setStatus] = useState<SaveStatus>('idle');
  const [error, setError] = useState('');

  const trimmed = value.trim();
  const valid = trimmed.length > 0;

  async function save() {
    if (!valid) return;
    setStatus('saving');
    setError('');
    try {
      await setClaudeSession(trimmed);
      setStatus('saved');
    } catch (e) {
      setStatus('error');
      setError(String(e));
    }
  }

  return (
    <div>
      <TextField
        label="Session key"
        multiline
        mono
        value={value}
        onChange={(v) => {
          setValue(v);
          setStatus('idle');
        }}
        placeholder="sessionKey value from your claude.ai cookies"
        hint={
          <>
            DevTools → Application → Cookies → claude.ai →{' '}
            <span className="font-mono">sessionKey</span>
          </>
        }
      />
      <SaveRow status={status} error={error} disabled={!valid} onSave={save} />
    </div>
  );
}
