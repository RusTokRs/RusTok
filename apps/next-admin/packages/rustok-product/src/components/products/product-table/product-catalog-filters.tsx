'use client';

import Link from 'next/link';
import { useTranslations } from '@rustok/next-fluent/client';
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import type {
  ProductCatalogFacet,
  ProductCatalogSearchOption
} from '../../../api/types';

interface ProductCatalogFiltersProps {
  categoryOptions: ProductCatalogSearchOption[];
  attributeOptions: ProductCatalogSearchOption[];
  facets: ProductCatalogFacet[];
  categoryId?: string;
  attributeFilters: string[];
  sortBy?: string;
  sortDirection?: string;
  perPage: number;
  routeQuery: Record<string, string>;
}

const FILTER_QUERY_KEYS = new Set([
  'page',
  'perPage',
  'category_id',
  'attribute_filters',
  'sort_by',
  'sort_direction'
]);

function hasFacetCode(entry: string, code: string): boolean {
  const equalsAt = entry.indexOf('=');
  return (
    equalsAt > 0 &&
    entry.slice(0, equalsAt).trim().toLowerCase() === code.toLowerCase()
  );
}

function hasFacetValue(entry: string, code: string, value: string): boolean {
  return (
    hasFacetCode(entry, code) &&
    entry.slice(entry.indexOf('=') + 1).trim() === value
  );
}

export function ProductCatalogFilters({
  categoryOptions,
  attributeOptions,
  facets,
  categoryId,
  attributeFilters,
  sortBy,
  sortDirection,
  perPage,
  routeQuery
}: ProductCatalogFiltersProps) {
  const t = useTranslations('product.catalog');
  const preservedQuery = Object.entries(routeQuery).filter(
    ([key]) => !FILTER_QUERY_KEYS.has(key)
  );

  function buildHref(nextFilters: string[], clearCatalogFilters = false) {
    const query = new URLSearchParams();
    for (const [key, value] of preservedQuery) {
      query.set(key, value);
    }
    if (!clearCatalogFilters) {
      if (categoryId) query.set('category_id', categoryId);
      if (sortBy) query.set('sort_by', sortBy);
      if (sortDirection) query.set('sort_direction', sortDirection);
    }
    query.set('page', '1');
    query.set('perPage', String(perPage));
    if (nextFilters.length > 0) {
      query.set('attribute_filters', nextFilters.join(';'));
    }
    return `/dashboard/product?${query.toString()}`;
  }

  function toggleFacetValue(code: string, value: string) {
    const entry = `${code}=${value}`;
    const nextFilters = attributeFilters.some((item) =>
      hasFacetValue(item, code, value)
    )
      ? attributeFilters.filter((item) => !hasFacetValue(item, code, value))
      : [...attributeFilters, entry];
    return buildHref(nextFilters);
  }

  function clearFacet(code: string) {
    return buildHref(
      attributeFilters.filter((entry) => !hasFacetCode(entry, code))
    );
  }

  return (
    <section className='space-y-4 rounded-lg border bg-card p-4'>
      <div className='flex flex-wrap items-start justify-between gap-3'>
        <div>
          <h2 className='text-sm font-semibold'>{t('filters')}</h2>
          <p className='text-muted-foreground text-xs'>{t('filtersDescription')}</p>
        </div>
        <Link
          href={buildHref([], true)}
          className='text-muted-foreground hover:text-foreground text-xs underline underline-offset-4'
        >
          {t('clear')}
        </Link>
      </div>

      <form
        method='get'
        action='/dashboard/product'
        className='grid gap-3 md:grid-cols-2 xl:grid-cols-4'
      >
        {preservedQuery.map(([key, value]) => (
          <input key={key} type='hidden' name={key} value={value} />
        ))}
        <input type='hidden' name='page' value='1' />
        <input type='hidden' name='perPage' value={String(perPage)} />

        <label className='grid gap-1.5 text-sm'>
          <span className='font-medium'>{t('category')}</span>
          <select
            key={categoryId ?? ''}
            name='category_id'
            defaultValue={categoryId ?? ''}
            className='border-input bg-background ring-offset-background focus-visible:ring-ring h-9 w-full rounded-md border px-3 text-sm shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2'
          >
            <option value=''>{t('allCategories')}</option>
            {categoryOptions.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>

        <label className='grid gap-1.5 text-sm'>
          <span className='font-medium'>{t('attributeFilters')}</span>
          <Input
            key={attributeFilters.join(';')}
            name='attribute_filters'
            defaultValue={attributeFilters.join(';')}
            placeholder={t('attributeFiltersPlaceholder')}
          />
        </label>

        <label className='grid gap-1.5 text-sm'>
          <span className='font-medium'>{t('sortBy')}</span>
          <select
            key={sortBy ?? 'published_at'}
            name='sort_by'
            defaultValue={sortBy ?? 'published_at'}
            className='border-input bg-background ring-offset-background focus-visible:ring-ring h-9 w-full rounded-md border px-3 text-sm shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2'
          >
            <option value='published_at'>{t('publishedAt')}</option>
            <option value='created_at'>{t('createdAt')}</option>
          </select>
        </label>

        <label className='grid gap-1.5 text-sm'>
          <span className='font-medium'>{t('sortDirection')}</span>
          <select
            key={sortDirection ?? 'desc'}
            name='sort_direction'
            defaultValue={sortDirection ?? 'desc'}
            className='border-input bg-background ring-offset-background focus-visible:ring-ring h-9 w-full rounded-md border px-3 text-sm shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2'
          >
            <option value='desc'>{t('newestFirst')}</option>
            <option value='asc'>{t('oldestFirst')}</option>
          </select>
        </label>

        <div className='flex flex-wrap items-end gap-2 md:col-span-2 xl:col-span-4'>
          <Button type='submit'>{t('apply')}</Button>
          {attributeOptions.length > 0 && (
            <p className='text-muted-foreground self-center text-xs'>
              {t('availableAttributes')}: {attributeOptions.map((item) => item.value).join(', ')}
            </p>
          )}
        </div>
      </form>

      {facets.length > 0 && (
        <div className='grid gap-3 border-t pt-3 md:grid-cols-2 xl:grid-cols-3'>
          {facets.map((facet) => (
            <details key={facet.code} className='rounded-md border p-3'>
              <summary className='flex cursor-pointer list-none items-center justify-between gap-2 text-sm font-medium'>
                <span>{facet.label}</span>
                <span className='text-muted-foreground text-xs'>
                  {facet.totalProducts}
                </span>
              </summary>
              <div className='mt-3 space-y-2'>
                {facet.values.length > 0 ? (
                  <ul className='space-y-1'>
                    {facet.values.map((item) => {
                      const selected = attributeFilters.some((entry) =>
                        hasFacetValue(entry, facet.code, item.value)
                      );
                      return (
                        <li key={item.value}>
                          <Link
                            href={toggleFacetValue(facet.code, item.value)}
                            aria-current={selected ? 'true' : undefined}
                            className={`flex items-center justify-between gap-2 rounded px-2 py-1 text-xs hover:bg-muted ${selected ? 'bg-muted font-semibold' : ''}`}
                          >
                            <span>{item.label || item.value}</span>
                            <span className='text-muted-foreground tabular-nums'>
                              {item.count}
                            </span>
                          </Link>
                        </li>
                      );
                    })}
                  </ul>
                ) : (
                  <p className='text-muted-foreground text-xs'>
                    {t('manualFacetHint')}
                  </p>
                )}
                {facet.isTruncated && (
                  <p className='text-muted-foreground text-xs'>
                    {t('truncated')}
                  </p>
                )}
                {attributeFilters.some((entry) =>
                  hasFacetCode(entry, facet.code)
                ) && (
                  <Link
                    href={clearFacet(facet.code)}
                    className='text-muted-foreground hover:text-foreground text-xs underline underline-offset-4'
                  >
                    {t('clearFacet')}
                  </Link>
                )}
              </div>
            </details>
          ))}
        </div>
      )}
    </section>
  );
}
