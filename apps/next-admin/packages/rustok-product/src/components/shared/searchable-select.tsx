'use client';

import * as React from 'react';
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/shadcn/popover';
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList
} from '@/shared/ui/shadcn/command';
import { Button } from '@/shared/ui/shadcn/button';
import { Check, ChevronsUpDown } from 'lucide-react';
import { cn } from '@/shared/lib/utils';

export interface SearchableSelectOption {
  value: string;
  label: string;
}

export interface SearchableSelectProps {
  id?: string;
  options: SearchableSelectOption[];
  /** Current selection; the `emptyValue` sentinel represents the empty choice. */
  value: string;
  /** Value that represents the empty choice. Defaults to `''`. */
  emptyValue?: string;
  /** Label shown for the empty choice (e.g. "Select a schema"). */
  emptyLabel: string;
  placeholder?: string;
  searchPlaceholder?: string;
  disabled?: boolean;
  className?: string;
  onChange: (value: string) => void;
}

/**
 * Generic searchable picker for admin reference lists (schemas, attributes,
 * …). Reference collections grow past the point where a plain `<select>` is
 * usable; this combobox filters the already-loaded options locally by their
 * rendered label, following the repo's Popover + Command convention.
 */
export function SearchableSelect({
  id,
  options,
  value,
  emptyValue = '',
  emptyLabel,
  placeholder,
  searchPlaceholder = 'Search...',
  disabled = false,
  className,
  onChange
}: SearchableSelectProps) {
  const [open, setOpen] = React.useState(false);

  const selectedLabel = React.useMemo(() => {
    if (!value || value === emptyValue) {
      return null;
    }
    return options.find((option) => option.value === value)?.label ?? null;
  }, [options, value, emptyValue]);

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          id={id}
          type='button'
          variant='outline'
          role='combobox'
          aria-expanded={open}
          disabled={disabled}
          className={cn(
            'h-9 w-full justify-between rounded-md font-normal text-sm',
            !selectedLabel && 'text-muted-foreground',
            className
          )}
        >
          <span className='truncate'>
            {selectedLabel ?? emptyLabel ?? placeholder}
          </span>
          <ChevronsUpDown className='text-muted-foreground h-3.5 w-3.5 shrink-0' />
        </Button>
      </PopoverTrigger>
      <PopoverContent className='w-[16rem] p-0' align='start'>
        <Command>
          <CommandInput placeholder={searchPlaceholder} />
          <CommandList>
            <CommandEmpty>No results found.</CommandEmpty>
            <CommandGroup>
              {emptyLabel ? (
                <CommandItem
                  value={`${emptyLabel} ${emptyValue}`}
                  onSelect={() => {
                    onChange(emptyValue);
                    setOpen(false);
                  }}
                >
                  <Check
                    className={cn(
                      'h-3.5 w-3.5',
                      !value || value === emptyValue
                        ? 'opacity-100'
                        : 'opacity-0'
                    )}
                  />
                  <span className='text-muted-foreground text-xs'>
                    {emptyLabel}
                  </span>
                </CommandItem>
              ) : null}
              {options.map((option) => (
                <CommandItem
                  key={option.value}
                  value={option.label}
                  onSelect={() => {
                    onChange(option.value);
                    setOpen(false);
                  }}
                >
                  <Check
                    className={cn(
                      'h-3.5 w-3.5',
                      value === option.value ? 'opacity-100' : 'opacity-0'
                    )}
                  />
                  <span className='truncate text-xs font-medium'>
                    {option.label}
                  </span>
                </CommandItem>
              ))}
            </CommandGroup>
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}
