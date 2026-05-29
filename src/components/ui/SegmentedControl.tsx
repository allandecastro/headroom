interface Option<T extends string> {
  value: T;
  label: string;
}

interface Props<T extends string> {
  options: Option<T>[];
  value: T;
  onChange: (value: T) => void;
  ariaLabel?: string;
}

// See DESIGN_SYSTEM.md § SegmentedControl.
export function SegmentedControl<T extends string>({
  options,
  value,
  onChange,
  ariaLabel,
}: Props<T>) {
  return (
    <div
      role="radiogroup"
      aria-label={ariaLabel}
      className="inline-flex border-hairline border-emphasis rounded-[5px] overflow-hidden"
    >
      {options.map((opt, i) => {
        const active = opt.value === value;
        return (
          <button
            key={opt.value}
            type="button"
            role="radio"
            aria-checked={active}
            onClick={() => onChange(opt.value)}
            className={`px-2.5 py-1 text-xxs ${i > 0 ? 'border-l border-hairline border-default' : ''} ${
              active ? 'bg-secondary text-fg-primary' : 'text-fg-secondary'
            }`}
          >
            {opt.label}
          </button>
        );
      })}
    </div>
  );
}
