import * as React from 'react';
import { Badge as ShadcnBadge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';

type IUBadgeVariant =
  | 'default'
  | 'secondary'
  | 'success'
  | 'warning'
  | 'danger';
type IUBadgeSize = 'sm' | 'md';

const variantClasses: Record<IUBadgeVariant, string> = {
  default: '',
  secondary: 'bg-secondary text-secondary-foreground border-transparent',
  success:
    'bg-emerald-100 text-emerald-700 border-transparent dark:bg-emerald-900/30 dark:text-emerald-400',
  warning:
    'bg-amber-100 text-amber-700 border-transparent dark:bg-amber-900/30 dark:text-amber-400',
  danger:
    'bg-rose-100 text-rose-700 border-transparent dark:bg-rose-900/30 dark:text-rose-400'
};

const sizeClasses: Record<IUBadgeSize, string> = {
  sm: 'px-1.5 py-0 text-[10px]',
  md: 'px-2 py-0.5 text-xs'
};

export interface BadgeProps extends React.ComponentProps<'span'> {
  variant?: IUBadgeVariant;
  size?: IUBadgeSize;
  dismissible?: boolean;
  onDismiss?: () => void;
}

export function Badge({
  variant = 'default',
  size = 'md',
  dismissible = false,
  onDismiss,
  children,
  className,
  ...props
}: BadgeProps) {
  const shadcnVariant =
    variant === 'default' ? 'default' : variant === 'secondary' ? 'secondary' : 'outline';

  return (
    <ShadcnBadge
      variant={shadcnVariant}
      className={cn(variantClasses[variant], sizeClasses[size], className)}
      {...props}
    >
      {children}
      {dismissible && (
        <button
          type='button'
          onClick={onDismiss}
          className='ml-1 rounded-full hover:opacity-70 focus:outline-none'
          aria-label='Dismiss'
        >
          ×
        </button>
      )}
    </ShadcnBadge>
  );
}
