interface Props {
  size?: number;
  className?: string;
}

export function BrandMark({ size = 24, className }: Props) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 64 64"
      fill="currentColor"
      role="img"
      aria-label="Headroom"
      className={className}
    >
      <rect x="10" y="11" width="44" height="4" rx="1" />
      <rect x="25" y="26" width="14" height="27" rx="1.5" />
    </svg>
  );
}
