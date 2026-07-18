/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

'use client';

import React from 'react';
import type { storefrontGraphql } from '@/shared/lib/graphql';

export type SearchStorefrontPageProps = {
  graphql: StorefrontGraphqlExecutor;
  token?: string | null;
  tenantSlug?: string | null;
  graphqlUrl?: string;
  locale?: string;
  initialQuery?: string;
  initialFilters?: Partial<SearchCatalogFilters>;
  categoryOptions?: SearchCatalogFilterOption[];
  attributeOptions?: SearchCatalogFilterOption[];
};

export type StorefrontGraphqlExecutor = typeof storefrontGraphql;

export type SearchCatalogFilterOption = {
  value: string;
  label: string;
};

export type SearchCatalogFilters = {
  channelId: string;
  categoryIds: string;
  attributeCode: string;
  attributeValues: string;
  attributeMin: string;
  attributeMax: string;
  sortAttributeCode: string;
  sortDesc: boolean;
};

type SearchSuggestion = {
  text: string;
  kind: string;
  locale: string | null;
  url: string | null;
  score: number;
};

type SearchResultItem = {
  id: string;
  entityType: string;
  sourceModule: string;
  title: string;
  snippet: string | null;
  score: number;
  locale: string | null;
  url: string | null;
  payload: string;
};

type SearchFacetGroup = {
  name: string;
  buckets: Array<{
    value: string;
    label: string | null;
    count: number;
  }>;
};

type SearchPreviewPayload = {
  queryLogId: string | null;
  presetKey: string | null;
  total: number;
  tookMs: number;
  engine: string;
  rankingProfile: string;
  items: SearchResultItem[];
  facets: SearchFacetGroup[];
};

type SearchFilterPreset = {
  key: string;
  label: string;
  entityTypes: string[];
  sourceModules: string[];
  statuses: string[];
  rankingProfile: string | null;
};

type StorefrontSearchResponse = {
  storefrontSearch: SearchPreviewPayload;
};

type StorefrontSuggestionsResponse = {
  storefrontSearchSuggestions: SearchSuggestion[];
};

type StorefrontFilterPresetsResponse = {
  storefrontSearchFilterPresets: SearchFilterPreset[];
};

const STOREFRONT_SEARCH_QUERY = `
  query StorefrontSearch($input: SearchPreviewInput!) {
    storefrontSearch(input: $input) {
      queryLogId
      presetKey
      total
      tookMs
      engine
      rankingProfile
      items {
        id
        entityType
        sourceModule
        title
        snippet
        score
        locale
        url
        payload
      }
      facets {
        name
        buckets {
          value
          label
          count
        }
      }
    }
  }
`;

const STOREFRONT_FILTER_PRESETS_QUERY = `
  query StorefrontSearchFilterPresets {
    storefrontSearchFilterPresets {
      key
      label
      entityTypes
      sourceModules
      statuses
      rankingProfile
    }
  }
`;

const STOREFRONT_SUGGESTIONS_QUERY = `
  query StorefrontSearchSuggestions($input: SearchSuggestionsInput!) {
    storefrontSearchSuggestions(input: $input) {
      text
      kind
      locale
      url
      score
    }
  }
`;

async function fetchStorefrontSearch(
  query: string,
  presetKey: string | null,
  filters: SearchCatalogFilters,
  props: SearchStorefrontPageProps
): Promise<SearchPreviewPayload> {
  const categoryIds = parseCsv(filters.categoryIds);
  const attributeValues = parseCsv(filters.attributeValues);
  const response = await props.graphql<
    StorefrontSearchResponse,
    { input: Record<string, unknown> }
  >({
    query: STOREFRONT_SEARCH_QUERY,
    variables: {
      input: {
        query,
        locale: props.locale || undefined,
        presetKey: presetKey || undefined,
        channelId: filters.channelId.trim() || undefined,
        categoryIds: categoryIds.length ? categoryIds : undefined,
        attributeFilters: filters.attributeCode.trim()
          ? [
              {
                attributeCode: filters.attributeCode.trim(),
                values: attributeValues.length ? attributeValues : undefined,
                min: filters.attributeMin.trim() || undefined,
                max: filters.attributeMax.trim() || undefined
              }
            ]
          : undefined,
        sortAttributeCode: filters.sortAttributeCode.trim() || undefined,
        sortDesc: filters.sortDesc || undefined,
        limit: 12,
        offset: 0
      }
    },
    token: props.token ?? undefined,
    tenant: props.tenantSlug ?? undefined,
    baseUrl: props.graphqlUrl
  });

  const payload = response.data?.storefrontSearch;
  if (!payload) {
    throw new Error('storefrontSearch payload is missing');
  }

  return payload;
}

async function fetchStorefrontFilterPresets(
  props: SearchStorefrontPageProps
): Promise<SearchFilterPreset[]> {
  const response = await props.graphql<StorefrontFilterPresetsResponse>({
    query: STOREFRONT_FILTER_PRESETS_QUERY,
    token: props.token ?? undefined,
    tenant: props.tenantSlug ?? undefined,
    baseUrl: props.graphqlUrl
  });

  return response.data?.storefrontSearchFilterPresets ?? [];
}

async function fetchStorefrontSuggestions(
  query: string,
  props: SearchStorefrontPageProps
): Promise<SearchSuggestion[]> {
  const response = await props.graphql<
    StorefrontSuggestionsResponse,
    { input: Record<string, unknown> }
  >({
    query: STOREFRONT_SUGGESTIONS_QUERY,
    variables: {
      input: {
        query,
        limit: 6
      }
    },
    token: props.token ?? undefined,
    tenant: props.tenantSlug ?? undefined,
    baseUrl: props.graphqlUrl
  });

  return response.data?.storefrontSearchSuggestions ?? [];
}

export function SearchStorefrontPage(
  props: SearchStorefrontPageProps
): React.JSX.Element {
  const [query, setQuery] = React.useState(props.initialQuery ?? '');
  const deferredQuery = React.useDeferredValue(query);
  const [results, setResults] =
    React.useState<SearchPreviewPayload | null>(null);
  const [suggestions, setSuggestions] = React.useState<SearchSuggestion[]>([]);
  const [presets, setPresets] = React.useState<SearchFilterPreset[]>([]);
  const [selectedPreset, setSelectedPreset] = React.useState('');
  const [catalogFilters, setCatalogFilters] =
    React.useState<SearchCatalogFilters>({
      channelId: props.initialFilters?.channelId ?? '',
      categoryIds: props.initialFilters?.categoryIds ?? '',
      attributeCode: props.initialFilters?.attributeCode ?? '',
      attributeValues: props.initialFilters?.attributeValues ?? '',
      attributeMin: props.initialFilters?.attributeMin ?? '',
      attributeMax: props.initialFilters?.attributeMax ?? '',
      sortAttributeCode: props.initialFilters?.sortAttributeCode ?? '',
      sortDesc: props.initialFilters?.sortDesc ?? false
    });
  const [searchError, setSearchError] = React.useState<string | null>(null);
  const [suggestionsError, setSuggestionsError] =
    React.useState<string | null>(null);
  const [presetsError, setPresetsError] = React.useState<string | null>(null);
  const [isLoadingResults, startResultsTransition] = React.useTransition();
  const [isLoadingSuggestions, startSuggestionsTransition] =
    React.useTransition();
  const [isLoadingPresets, startPresetsTransition] = React.useTransition();

  const loadResults = React.useEffectEvent(
    async (
      nextQuery: string,
      nextPreset: string | null,
      filters: SearchCatalogFilters
    ) => {
      const trimmed = nextQuery.trim();
      if (!trimmed) {
        setResults(null);
        setSearchError(null);
        return;
      }

      try {
        const payload = await fetchStorefrontSearch(
          trimmed,
          nextPreset,
          filters,
          props
        );
        startResultsTransition(() => {
          setResults(payload);
          setSearchError(null);
        });
      } catch (error) {
        startResultsTransition(() => {
          setSearchError(
            error instanceof Error
              ? error.message
              : 'Failed to load storefront search'
          );
        });
      }
    }
  );

  const loadPresets = React.useEffectEvent(async () => {
    try {
      const payload = await fetchStorefrontFilterPresets(props);
      startPresetsTransition(() => {
        setPresets(payload);
        setPresetsError(null);
      });
    } catch (error) {
      startPresetsTransition(() => {
        setPresetsError(
          error instanceof Error ? error.message : 'Failed to load presets'
        );
      });
    }
  });

  const loadSuggestions = React.useEffectEvent(async (nextQuery: string) => {
    const trimmed = nextQuery.trim();
    if (trimmed.length < 2) {
      startSuggestionsTransition(() => {
        setSuggestions([]);
        setSuggestionsError(null);
      });
      return;
    }

    try {
      const payload = await fetchStorefrontSuggestions(trimmed, props);
      startSuggestionsTransition(() => {
        setSuggestions(payload);
        setSuggestionsError(null);
      });
    } catch (error) {
      startSuggestionsTransition(() => {
        setSuggestionsError(
          error instanceof Error
            ? error.message
            : 'Failed to load storefront suggestions'
        );
      });
    }
  });

  React.useEffect(() => {
    void loadResults(
      props.initialQuery ?? '',
      selectedPreset || null,
      catalogFilters
    );
  }, [loadResults, props.initialQuery, selectedPreset]);

  React.useEffect(() => {
    void loadSuggestions(deferredQuery);
  }, [deferredQuery, loadSuggestions]);

  React.useEffect(() => {
    void loadPresets();
  }, [loadPresets]);

  return (
    <section className='rounded-[1.75rem] border border-zinc-300 p-7 dark:border-zinc-700'>
      <div className='text-xs uppercase tracking-[0.14em] text-zinc-500'>
        search
      </div>
      <h2 className='mt-2.5 text-3xl font-semibold'>
        Search experiences start here
      </h2>
      <p className='mt-3 max-w-[47.5rem] text-zinc-600 dark:text-zinc-300'>
        Next storefront package now follows the same live search and
        autocomplete contract as the Leptos storefront module.
      </p>

      <form
        className='mt-6 grid gap-3'
        onSubmit={(event) => {
          event.preventDefault();
          void loadResults(query, selectedPreset || null, catalogFilters);
        }}
      >
        <div className='flex flex-wrap gap-3'>
          <input
            className='min-w-0 flex-[1_1_360px] rounded-[1.125rem] border border-zinc-300 bg-white px-4 py-3.5 text-[15px] outline-none transition focus:border-zinc-900 focus:ring-2 focus:ring-zinc-900/10 dark:border-zinc-700 dark:bg-zinc-950 dark:focus:border-zinc-100'
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder='Search products and published content'
          />
          <button
            className='rounded-[1.125rem] bg-zinc-900 px-[1.125rem] py-3.5 font-semibold text-zinc-50 transition hover:bg-zinc-800 dark:bg-zinc-100 dark:text-zinc-950 dark:hover:bg-white'
            type='submit'
          >
            Search
          </button>
        </div>

        <div className='rounded-[1.25rem] border border-zinc-200 p-4 dark:border-zinc-800'>
          <div className='flex justify-between gap-3'>
            <strong>Filter presets</strong>
            <span className='text-xs text-zinc-500'>
              {isLoadingPresets ? 'loading…' : 'surface defaults'}
            </span>
          </div>
          {presetsError ? (
            <p className='mt-2.5 text-red-700 dark:text-red-400'>
              {presetsError}
            </p>
          ) : presets.length === 0 ? (
            <p className='mt-2.5 text-zinc-500'>No presets configured yet.</p>
          ) : (
            <div className='mt-3 flex flex-wrap gap-2'>
              {presets.map((preset) => {
                const isSelected = selectedPreset === preset.key;
                return (
                  <button
                    key={preset.key}
                    className={
                      isSelected
                        ? 'rounded-full border border-teal-700 bg-teal-100 px-3 py-2 text-xs font-semibold text-teal-900 transition dark:border-teal-400 dark:bg-teal-950 dark:text-teal-100'
                        : 'rounded-full border border-zinc-300 bg-white px-3 py-2 text-xs font-semibold text-zinc-800 transition hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-950 dark:text-zinc-100 dark:hover:bg-zinc-900'
                    }
                    onClick={() => {
                      const nextPreset = isSelected ? '' : preset.key;
                      setSelectedPreset(nextPreset);
                      void loadResults(
                        query,
                        nextPreset || null,
                        catalogFilters
                      );
                    }}
                    type='button'
                  >
                    {preset.label}
                  </button>
                );
              })}
            </div>
          )}
        </div>

        <details className='rounded-xl border border-zinc-200 p-4 dark:border-zinc-800'>
          <summary className='cursor-pointer font-semibold'>
            Catalog filters and sorting
          </summary>
          <div className='mt-3.5 grid grid-cols-[repeat(auto-fit,minmax(220px,1fr))] gap-3'>
            <CatalogField
              label='Channel ID'
              value={catalogFilters.channelId}
              onChange={(channelId) =>
                setCatalogFilters((current) => ({ ...current, channelId }))
              }
            />
            <CatalogField
              label='Category IDs (CSV)'
              value={catalogFilters.categoryIds}
              onChange={(categoryIds) =>
                setCatalogFilters((current) => ({ ...current, categoryIds }))
              }
              options={props.categoryOptions}
            />
            <CatalogField
              label='Attribute code'
              value={catalogFilters.attributeCode}
              onChange={(attributeCode) =>
                setCatalogFilters((current) => ({
                  ...current,
                  attributeCode
                }))
              }
              options={props.attributeOptions}
            />
            <CatalogField
              label='Attribute values (CSV)'
              value={catalogFilters.attributeValues}
              onChange={(attributeValues) =>
                setCatalogFilters((current) => ({
                  ...current,
                  attributeValues
                }))
              }
            />
            <CatalogField
              label='Minimum'
              value={catalogFilters.attributeMin}
              onChange={(attributeMin) =>
                setCatalogFilters((current) => ({
                  ...current,
                  attributeMin
                }))
              }
            />
            <CatalogField
              label='Maximum'
              value={catalogFilters.attributeMax}
              onChange={(attributeMax) =>
                setCatalogFilters((current) => ({
                  ...current,
                  attributeMax
                }))
              }
            />
            <CatalogField
              label='Sort attribute code'
              value={catalogFilters.sortAttributeCode}
              onChange={(sortAttributeCode) =>
                setCatalogFilters((current) => ({
                  ...current,
                  sortAttributeCode
                }))
              }
              options={props.attributeOptions}
            />
            <label className='flex items-center gap-2 text-sm'>
              <input
                type='checkbox'
                checked={catalogFilters.sortDesc}
                onChange={(event) =>
                  setCatalogFilters((current) => ({
                    ...current,
                    sortDesc: event.target.checked
                  }))
                }
              />
              Descending order
            </label>
          </div>
        </details>

        <div className='rounded-[1.25rem] border border-zinc-200 p-4 dark:border-zinc-800'>
          <div className='flex justify-between gap-3'>
            <strong>Suggestions</strong>
            <span className='text-xs text-zinc-500'>
              {isLoadingSuggestions ? 'loading…' : 'autocomplete'}
            </span>
          </div>
          {suggestionsError ? (
            <p className='mt-2.5 text-red-700 dark:text-red-400'>
              {suggestionsError}
            </p>
          ) : suggestions.length === 0 ? (
            <p className='mt-2.5 text-zinc-500'>
              Type at least 2 characters to load query and document suggestions.
            </p>
          ) : (
            <div className='mt-3 grid gap-2'>
              {suggestions.map((suggestion) => (
                <button
                  key={`${suggestion.kind}:${suggestion.text}`}
                  className='w-full rounded-2xl border border-zinc-200 bg-white p-3.5 text-left transition hover:border-zinc-400 hover:bg-zinc-50 dark:border-zinc-800 dark:bg-zinc-950 dark:hover:border-zinc-600 dark:hover:bg-zinc-900'
                  onClick={() => {
                    if (suggestion.kind === 'document' && suggestion.url) {
                      window.location.href = suggestion.url;
                      return;
                    }
                    setQuery(suggestion.text);
                    void loadResults(
                      suggestion.text,
                      selectedPreset || null,
                      catalogFilters
                    );
                  }}
                  type='button'
                >
                  <div className='font-semibold'>{suggestion.text}</div>
                  <div className='mt-1 text-xs text-zinc-500'>
                    {suggestion.kind}
                    {suggestion.locale ? ` • ${suggestion.locale}` : ''}
                  </div>
                </button>
              ))}
            </div>
          )}
        </div>
      </form>

      <div className='mt-6 rounded-[1.25rem] border border-zinc-200 p-5 dark:border-zinc-800'>
        <div className='flex flex-wrap justify-between gap-3'>
          <strong>Results</strong>
          <span className='text-xs text-zinc-500'>
            {isLoadingResults
              ? 'loading…'
              : results
                ? `${results.total} hits via ${results.engine} (${results.rankingProfile})`
                : 'idle'}
          </span>
        </div>
        {searchError ? (
          <p className='mt-3 text-red-700 dark:text-red-400'>{searchError}</p>
        ) : !results ? (
          <p className='mt-3 text-zinc-500'>
            Submit a query to preview the live storefront search contract in
            Next.
          </p>
        ) : (
          <div className='mt-4 grid gap-3.5'>
            <div className='text-zinc-600 dark:text-zinc-300'>
              {results.total} results in {results.tookMs} ms
              {results.presetKey ? ` • preset ${results.presetKey}` : ''}
            </div>
            {results.facets.length ? (
              <div className='flex flex-wrap gap-2'>
                {results.facets.flatMap((facet) =>
                  facet.buckets.map((bucket) => (
                    <span
                      key={`${facet.name}:${bucket.value}`}
                      className='border border-zinc-300 px-[9px] py-1.5 text-xs dark:border-zinc-700'
                    >
                      {bucket.label || bucket.value} ({bucket.count})
                    </span>
                  ))
                )}
              </div>
            ) : null}
            {results.items.map((item) => (
              <article
                key={item.id}
                className='rounded-[1.125rem] border border-zinc-200 p-4 dark:border-zinc-800'
              >
                <div className='text-xs uppercase text-zinc-500'>
                  {item.entityType} • {item.sourceModule}
                </div>
                <h3 className='mt-2 text-lg font-semibold'>{item.title}</h3>
                <p className='mt-2 text-zinc-600 dark:text-zinc-300'>
                  {item.snippet ?? 'No snippet returned.'}
                </p>
              </article>
            ))}
          </div>
        )}
      </div>
    </section>
  );
}

function parseCsv(value: string): string[] {
  return value
    .split(',')
    .map((segment) => segment.trim())
    .filter(Boolean);
}

function CatalogField(props: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  options?: SearchCatalogFilterOption[];
}): React.JSX.Element {
  const listId = React.useId();
  return (
    <label className='grid gap-1.5 text-[13px]'>
      <span>{props.label}</span>
      <input
        className='border border-zinc-300 bg-white px-3 py-2.5 outline-none transition focus:border-zinc-900 focus:ring-2 focus:ring-zinc-900/10 dark:border-zinc-700 dark:bg-zinc-950 dark:focus:border-zinc-100'
        value={props.value}
        onChange={(event) => props.onChange(event.target.value)}
        list={props.options?.length ? listId : undefined}
      />
      {props.options?.length ? (
        <datalist id={listId}>
          {props.options.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </datalist>
      ) : null}
    </label>
  );
}

export default SearchStorefrontPage;
