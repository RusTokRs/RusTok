import * as React from 'react';
import { Switch as ShadcnSwitch } from '@/components/ui/switch';
import { cn } from '@/lib/utils';

type IUSwitchSize = 'sm' | 'md';

const sizeClasses: Record<IUSwitchSize, string> = {
  sm: 'h-4 w-7 [&>span]:h-3 [&>span]:w-3',
  md: 'h-5 w-9 [&>span]:h-4 [&>span]:w-4'
};

export interface SwitchProps {
  checked?: boolean;
  disabled?: boolean;
  size?: IUSwitchSize;
  onCheckedChange?: (checked: boolean) => void;
  className?: string;
  id?: string;
}

export function Switch({
  checked,
  disabled,
  size = 'md',
  onCheckedChange,
  className,
  id
}: SwitchProps) {
  return (
    <ShadcnSwitch
      id={id}
      checked={checked}
      disabled={disabled}
      onCheckedChange={onCheckedChange}
      className={cn(sizeClasses[size], className)}
    />
  );
}
