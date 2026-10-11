import * as React from 'react';
import type { DataTableEmptyStateProps } from './types';

/** The standard empty state: an optional icon above a copy message. */
export function DataTableEmptyState({ icon, message, className }: DataTableEmptyStateProps) {
  return (
    <div
      className={`flex flex-col items-center justify-center gap-1.5 ${className ?? ''}`}
      role='status'
    >
      {icon ? (
        <span aria-hidden='true' className='text-muted-foreground/50 [&_svg]:h-8 [&_svg]:w-8'>
          {icon}
        </span>
      ) : null}
      <p>{message}</p>
    </div>
  );
}
