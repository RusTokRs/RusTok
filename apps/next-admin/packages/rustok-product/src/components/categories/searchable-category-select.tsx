'use client';

import { cn } from '@/shared/lib/utils';
import type { SearchableSelectProps } from '../shared/searchable-select';
import { SearchableSelect } from '../shared/searchable-select';

export interface SearchableCategoryOption {
  id: string;
  label: string;
}

export interface SearchableCategorySelectProps
  extends Omit<SearchableSelectProps, 'options' | 'value'> {
  options: SearchableCategoryOption[];
  /** Current selection; the sentinel `emptyValue` represents "no category". */
  value: string;
}

/**
 * Searchable category picker for the admin UIs.
 *
 * Category trees grow into the hundreds of rows, where a plain `<Select>`
 * stops being usable. This combobox keeps the full list server fetch (the
 * catalog categories query has no search argument yet) and filters locally
 * over the already-loaded options, matching by the rendered label — which is
 * the category path, name, or code depending on availability.
 */
export function SearchableCategorySelect({
  options,
  value,
  searchPlaceholder = 'Search categories...',
  className,
  ...rest
}: SearchableCategorySelectProps) {
  return (
    <SearchableSelect
      options={options.map((option) => ({
        value: option.id,
        label: option.label
      }))}
      value={value}
      searchPlaceholder={searchPlaceholder}
      className={cn('rounded-xl text-xs', className)}
      {...rest}
    />
  );
}
