import { useState } from 'react';
import { setClaudeSession } from '../../lib/ipc';
import { SaveRow } from './SaveRow';
import type { SaveStatus } from './SaveRow';

const inputClass =
  'w-full bg-secondary border-hairline border-emphasis rounded-[6px] px-2.5 py-2 text-[12px] text-fg-primary placeholder:text-fg-quaternary font-mono outline-none focus:border-emphasis';

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
      <label htmlFor="claude-session" className="block mb-1.5 text-[11px] text-fg-secondary">
        Session key
      </label>
      <textarea
        id="claude-session"
        rows={3}
        spellCheck={false}
        value={value}
        onChange={(e) => {
          setValue(e.target.value);
          setStatus('idle');
        }}
        placeholder="sessionKey value from your claude.ai cookies"
        className={`${inputClass} resize-none`}
      />
      <p className="mt-1.5 text-2xs text-fg-tertiary">
        DevTools → Application → Cookies → claude.ai → <span className="font-mono">sessionKey</span>
      </p>
      <SaveRow status={status} error={error} disabled={!valid} onSave={save} />
    </div>
  );
}
