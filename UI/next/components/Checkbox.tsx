import * as React from 'react';
import { Checkbox as ShadcnCheckbox } from '@/components/ui/checkbox';
import { cn } from '@/lib/utils';

export interface CheckboxProps {
  checked?: boolean;
  indeterminate?: boolean;
  disabled?: boolean;
  onCheckedChange?: (checked: boolean) => void;
  className?: string;
  id?: string;
}

export function Checkbox({
  checked,
  indeterminate,
  disabled,
  onCheckedChange,
  className,
  id
}: CheckboxProps) {
  const checkedState = indeterminate ? 'indeterminate' : checked;

  return (
    <ShadcnCheckbox
      id={id}
      checked={checkedState}
      disabled={disabled}
      onCheckedChange={(value) => {
        if (onCheckedChange && typeof value === 'boolean') {
          onCheckedChange(value);
        }
      }}
      className={cn(className)}
    />
  );
}
