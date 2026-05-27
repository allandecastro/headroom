import { useId } from 'react';
import type { ReactNode } from 'react';

interface Props {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  hint?: ReactNode;
  error?: string;
  multiline?: boolean;
  password?: boolean;
  mono?: boolean;
}

const inputClass =
  'w-full rounded-[6px] border-hairline border-emphasis bg-secondary px-2.5 py-2 text-[12px] text-fg-primary outline-none placeholder:text-fg-quaternary focus:border-emphasis';

// Labelled text input/textarea used by the onboarding paste forms.
export function TextField({
  label,
  value,
  onChange,
  placeholder,
  hint,
  error,
  multiline = false,
  password = false,
  mono = false,
}: Props) {
  const id = useId();
  const cls = `${inputClass}${mono ? ' font-mono' : ''}${multiline ? ' resize-none' : ''}`;
  return (
    <div>
      <label htmlFor={id} className="mb-1.5 block text-[11px] text-fg-secondary">
        {label}
      </label>
      {multiline ? (
        <textarea
          id={id}
          rows={3}
          spellCheck={false}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          className={cls}
        />
      ) : (
        <input
          id={id}
          type={password ? 'password' : 'text'}
          spellCheck={false}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          className={cls}
        />
      )}
      {error ? (
        <p className="mt-1.5 text-2xs text-state-crit-text dark:text-state-crit-text-dark">
          {error}
        </p>
      ) : (
        hint && <p className="mt-1.5 text-2xs text-fg-tertiary">{hint}</p>
      )}
    </div>
  );
}
