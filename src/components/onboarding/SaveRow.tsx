import { Button } from '../ui/Button';

export type SaveStatus = 'idle' | 'saving' | 'saved' | 'error';

interface Props {
  status: SaveStatus;
  error?: string;
  disabled?: boolean;
  onSave: () => void;
}

// Save button + inline status, shared by the per-service paste forms.
export function SaveRow({ status, error, disabled, onSave }: Props) {
  return (
    <div className="flex items-center gap-3 mt-2.5">
      <Button variant="primary" onClick={onSave} disabled={disabled || status === 'saving'}>
        {status === 'saving' ? 'Saving…' : 'Save'}
      </Button>
      {status === 'saved' && (
        <span className="inline-flex items-center gap-1.5 text-[10px] text-state-ok-text dark:text-state-ok-text-dark">
          <span className="w-[5px] h-[5px] rounded-full bg-state-ok-fill dark:bg-state-ok-fill-dark" />
          Saved
        </span>
      )}
      {status === 'error' && error && (
        <span className="text-2xs text-state-crit-text dark:text-state-crit-text-dark">
          {error}
        </span>
      )}
    </div>
  );
}
