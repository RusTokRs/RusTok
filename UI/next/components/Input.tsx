import * as React from 'react';
import { Input as ShadcnInput } from '@/components/ui/input';
import { cn } from '@/lib/utils';

type IUInputSize = 'sm' | 'md' | 'lg';

const sizeClasses: Record<IUInputSize, string> = {
  sm: 'h-8 text-xs px-2',
  md: 'h-9 text-sm px-3',
  lg: 'h-10 text-base px-4'
};

export interface InputProps
  extends Omit<React.ComponentProps<'input'>, 'size' | 'prefix'> {
  size?: IUInputSize;
  invalid?: boolean;
  prefix?: React.ReactNode;
  suffix?: React.ReactNode;
}

export function Input({
  size = 'md',
  invalid = false,
  prefix,
  suffix,
  className,
  ...props
}: InputProps) {
  if (!prefix && !suffix) {
    return (
      <ShadcnInput
        aria-invalid={invalid || undefined}
        className={cn(sizeClasses[size], className)}
        {...props}
      />
    );
  }

  return (
    <div className='relative flex items-center'>
      {prefix && (
        <span className='pointer-events-none absolute left-3 text-muted-foreground'>
          {prefix}
        </span>
      )}
      <ShadcnInput
        aria-invalid={invalid || undefined}
        className={cn(sizeClasses[size], prefix && 'pl-9', suffix && 'pr-9', className)}
        {...props}
      />
      {suffix && (
        <span className='pointer-events-none absolute right-3 text-muted-foreground'>
          {suffix}
        </span>
      )}
    </div>
  );
}
