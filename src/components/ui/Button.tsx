import type { ComponentPropsWithoutRef } from 'react';

interface Props extends ComponentPropsWithoutRef<'button'> {
  variant?: 'default' | 'primary';
}

// See DESIGN_SYSTEM.md § Button and docs/mockups/style.css (.btn / .btn-primary).
export function Button({ variant = 'default', className = '', ...rest }: Props) {
  const base =
    'inline-flex items-center gap-1.5 text-[11px] px-3 py-[5px] rounded-[5px] border-hairline disabled:opacity-40 disabled:cursor-default';
  const look =
    variant === 'primary'
      ? 'bg-state-ok-fill dark:bg-state-ok-fill-dark border-state-ok-fill dark:border-state-ok-fill-dark text-[#fafafa]'
      : 'bg-transparent border-emphasis text-fg-primary hover:bg-secondary';
  return <button className={`${base} ${look} ${className}`} {...rest} />;
}
