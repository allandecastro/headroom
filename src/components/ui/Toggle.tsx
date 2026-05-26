// Toggle pill — 26×15 px, knob 11×11 px.
// Off: border-emphasis background. On: ok-fill background.
// See DESIGN_SYSTEM.md § Toggle and docs/mockups/03-settings.html (.toggle).

interface Props {
  checked: boolean;
  onChange: (checked: boolean) => void;
  ariaLabel?: string;
}

export function Toggle({ checked, onChange, ariaLabel }: Props) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel}
      onClick={() => onChange(!checked)}
      className="relative flex-shrink-0 focus-visible:outline-none"
      style={{
        width: 26,
        height: 15,
        borderRadius: 999,
        background: checked ? 'var(--ok-fill, #6a8e4a)' : 'var(--border-emphasis)',
        transition: 'background 0.15s',
        padding: 0,
        border: 'none',
        cursor: 'default',
      }}
    >
      <span
        aria-hidden
        style={{
          position: 'absolute',
          top: 2,
          left: checked ? 13 : 2,
          width: 11,
          height: 11,
          borderRadius: '50%',
          background: '#fafafa',
          transition: 'left 0.15s',
        }}
      />
    </button>
  );
}
