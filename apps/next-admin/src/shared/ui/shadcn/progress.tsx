'use client';

import * as React from 'react';
import * as ProgressPrimitive from '@radix-ui/react-progress';

import { cn } from '@/shared/lib/utils';

function Progress({
  className,
  value,
  ...props
}: React.ComponentProps<typeof ProgressPrimitive.Root>) {
  const numericValue =
    typeof value === 'number' && Number.isFinite(value) ? value : 0;
  const progressValue = Math.min(100, Math.max(0, numericValue));

  return (
    <ProgressPrimitive.Root
      data-slot='progress'
      className={cn(
        'bg-primary/20 relative h-2 w-full overflow-hidden rounded-full',
        className
      )}
      value={progressValue}
      {...props}
    >
      <svg
        aria-hidden='true'
        className='block h-full w-full'
        preserveAspectRatio='none'
        viewBox='0 0 100 2'
      >
        <rect
          className='fill-primary transition-all'
          height='2'
          width={progressValue}
          x='0'
          y='0'
        />
      </svg>
    </ProgressPrimitive.Root>
  );
}

export { Progress };
