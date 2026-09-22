import * as React from 'react';
import { Textarea as ShadcnTextarea } from '@/components/ui/textarea';
import { cn } from '@/lib/utils';

type IUTextareaSize = 'sm' | 'md' | 'lg';

const sizeClasses: Record<IUTextareaSize, string> = {
  sm: 'text-xs px-2 py-1.5',
  md: 'text-sm px-3 py-2',
  lg: 'text-base px-4 py-3'
};

export interface TextareaProps
  extends Omit<React.ComponentProps<'textarea'>, 'size'> {
  size?: IUTextareaSize;
  invalid?: boolean;
}

export function Textarea({
  size = 'md',
  invalid = false,
  className,
  ...props
}: TextareaProps) {
  return (
    <ShadcnTextarea
      aria-invalid={invalid || undefined}
      className={cn(sizeClasses[size], className)}
      {...props}
    />
  );
}
