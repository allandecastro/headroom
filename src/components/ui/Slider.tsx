interface Props {
  value: number; // 0–100, where 0 means "off"
  onChange: (value: number) => void;
  accent: string; // CSS color for the track/thumb (matches the alert color)
  ariaLabel?: string;
}

// A percentage slider (step 5) with a live "Off / NN%" readout. Used for the
// notification thresholds so the user can dial them in freely.
export function Slider({ value, onChange, accent, ariaLabel }: Props) {
  return (
    <div className="flex w-[150px] items-center gap-2">
      <input
        type="range"
        min={0}
        max={100}
        step={5}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        aria-label={ariaLabel}
        className="h-1 flex-1 cursor-pointer"
        style={{ accentColor: accent }}
      />
      <span className="w-7 text-right text-[10.5px] tabular-nums text-fg-secondary">
        {value === 0 ? 'Off' : `${value}%`}
      </span>
    </div>
  );
}
