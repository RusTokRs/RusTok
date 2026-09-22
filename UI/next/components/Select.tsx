import * as React from 'react';
import {
  Select as ShadcnSelect,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/components/ui/select';
import { cn } from '@/lib/utils';

type IUSelectSize = 'sm' | 'md' | 'lg';

const sizeClasses: Record<IUSelectSize, string> = {
  sm: 'h-8 text-xs',
  md: 'h-9 text-sm',
  lg: 'h-10 text-base'
};

export interface SelectOption {
  value: string;
  label: string;
  disabled?: boolean;
}

export interface SelectProps {
  size?: IUSelectSize;
  disabled?: boolean;
  invalid?: boolean;
  options: SelectOption[];
  placeholder?: string;
  value?: string;
  onValueChange?: (value: string) => void;
  className?: string;
}

export function Select({
  size = 'md',
  disabled = false,
  invalid = false,
  options,
  placeholder,
  value,
  onValueChange,
  className
}: SelectProps) {
  return (
    <ShadcnSelect value={value} onValueChange={onValueChange} disabled={disabled}>
      <SelectTrigger
        aria-invalid={invalid || undefined}
        className={cn(sizeClasses[size], className)}
      >
        <SelectValue placeholder={placeholder} />
      </SelectTrigger>
      <SelectContent>
        {options.map((opt) => (
          <SelectItem key={opt.value} value={opt.value} disabled={opt.disabled}>
            {opt.label}
          </SelectItem>
        ))}
      </SelectContent>
    </ShadcnSelect>
  );
}
