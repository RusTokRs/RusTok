import * as React from 'react';
import { cn } from '@/lib/utils';

type IUSpinnerSize = 'sm' | 'md' | 'lg';

const sizeClasses: Record<IUSpinnerSize, string> = {
  sm: 'h-4 w-4 border-2',
  md: 'h-6 w-6 border-2',
  lg: 'h-8 w-8 border-[3px]'
};

export interface SpinnerProps {
  size?: IUSpinnerSize;
  className?: string;
}

export function Spinner({ size = 'md', className }: SpinnerProps) {
  return (
    <span
      role='status'
      aria-label='Loading'
      className={cn(
        'inline-block rounded-full border-current border-t-transparent animate-spin',
        sizeClasses[size],
        className
      )}
    />
  );
}
