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

import { storefrontGraphql } from '../../../src/shared/lib/graphql';

export type SearchStorefrontPageProps = {
  token?: string | null;
  tenantSlug?: string | null;
  graphqlUrl?: string;
  locale?: string;
  initialQuery?: string;
  initialFilters?: Partial<SearchCatalogFilters>;
  categoryOptions?: SearchCatalogFilterOption[];
  attributeOptions?: SearchCatalogFilterOption[];
};

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
  props: SearchStorefrontPageProps,
): Promise<SearchPreviewPayload> {
  const categoryIds = parseCsv(filters.categoryIds);
  const attributeValues = parseCsv(filters.attributeValues);
  const response = await storefrontGraphql<StorefrontSearchResponse, { input: Record<string, unknown> }>({
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
                max: filters.attributeMax.trim() || undefined,
              },
            ]
          : undefined,
        sortAttributeCode: filters.sortAttributeCode.trim() || undefined,
        sortDesc: filters.sortDesc || undefined,
        limit: 12,
        offset: 0,
      },
    },
    token: props.token ?? undefined,
    tenant: props.tenantSlug ?? undefined,
    baseUrl: props.graphqlUrl,
  });

  const payload = response.data?.storefrontSearch;
  if (!payload) {
    throw new Error('storefrontSearch payload is missing');
  }

  return payload;
}

async function fetchStorefrontFilterPresets(
  props: SearchStorefrontPageProps,
): Promise<SearchFilterPreset[]> {
  const response = await storefrontGraphql<StorefrontFilterPresetsResponse>({
    query: STOREFRONT_FILTER_PRESETS_QUERY,
    token: props.token ?? undefined,
    tenant: props.tenantSlug ?? undefined,
    baseUrl: props.graphqlUrl,
  });

  return response.data?.storefrontSearchFilterPresets ?? [];
}

async function fetchStorefrontSuggestions(
  query: string,
  props: SearchStorefrontPageProps,
): Promise<SearchSuggestion[]> {
  const response = await storefrontGraphql<
    StorefrontSuggestionsResponse,
    { input: Record<string, unknown> }
  >({
    query: STOREFRONT_SUGGESTIONS_QUERY,
    variables: {
      input: {
        query,
        limit: 6,
      },
    },
    token: props.token ?? undefined,
    tenant: props.tenantSlug ?? undefined,
    baseUrl: props.graphqlUrl,
  });

  return response.data?.storefrontSearchSuggestions ?? [];
}

export function SearchStorefrontPage(props: SearchStorefrontPageProps): React.JSX.Element {
  const [query, setQuery] = React.useState(props.initialQuery ?? '');
  const deferredQuery = React.useDeferredValue(query);
  const [results, setResults] = React.useState<SearchPreviewPayload | null>(null);
  const [suggestions, setSuggestions] = React.useState<SearchSuggestion[]>([]);
  const [presets, setPresets] = React.useState<SearchFilterPreset[]>([]);
  const [selectedPreset, setSelectedPreset] = React.useState('');
  const [catalogFilters, setCatalogFilters] = React.useState<SearchCatalogFilters>({
    channelId: props.initialFilters?.channelId ?? '',
    categoryIds: props.initialFilters?.categoryIds ?? '',
    attributeCode: props.initialFilters?.attributeCode ?? '',
    attributeValues: props.initialFilters?.attributeValues ?? '',
    attributeMin: props.initialFilters?.attributeMin ?? '',
    attributeMax: props.initialFilters?.attributeMax ?? '',
    sortAttributeCode: props.initialFilters?.sortAttributeCode ?? '',
    sortDesc: props.initialFilters?.sortDesc ?? false,
  });
  const [searchError, setSearchError] = React.useState<string | null>(null);
  const [suggestionsError, setSuggestionsError] = React.useState<string | null>(null);
  const [presetsError, setPresetsError] = React.useState<string | null>(null);
  const [isLoadingResults, startResultsTransition] = React.useTransition();
  const [isLoadingSuggestions, startSuggestionsTransition] = React.useTransition();
  const [isLoadingPresets, startPresetsTransition] = React.useTransition();

  const loadResults = React.useEffectEvent(async (
    nextQuery: string,
    nextPreset: string | null,
    filters: SearchCatalogFilters,
  ) => {
    const trimmed = nextQuery.trim();
    if (!trimmed) {
      setResults(null);
      setSearchError(null);
      return;
    }

    try {
      const payload = await fetchStorefrontSearch(trimmed, nextPreset, filters, props);
      startResultsTransition(() => {
        setResults(payload);
        setSearchError(null);
      });
    } catch (error) {
      startResultsTransition(() => {
        setSearchError(error instanceof Error ? error.message : 'Failed to load storefront search');
      });
    }
  });

  const loadPresets = React.useEffectEvent(async () => {
    try {
      const payload = await fetchStorefrontFilterPresets(props);
      startPresetsTransition(() => {
        setPresets(payload);
        setPresetsError(null);
      });
    } catch (error) {
      startPresetsTransition(() => {
        setPresetsError(error instanceof Error ? error.message : 'Failed to load presets');
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
          error instanceof Error ? error.message : 'Failed to load storefront suggestions',
        );
      });
    }
  });

  React.useEffect(() => {
    void loadResults(props.initialQuery ?? '', selectedPreset || null, catalogFilters);
  }, [loadResults, props.initialQuery, selectedPreset]);

  React.useEffect(() => {
    void loadSuggestions(deferredQuery);
  }, [deferredQuery, loadSuggestions]);

  React.useEffect(() => {
    void loadPresets();
  }, [loadPresets]);

  return (
    <section style={{ border: '1px solid #d4d4d8', borderRadius: 28, padding: 28 }}>
      <div
        style={{ fontSize: 12, textTransform: 'uppercase', letterSpacing: '0.14em', color: '#71717a' }}
      >
        search
      </div>
      <h2 style={{ marginTop: 10, fontSize: 32 }}>Search experiences start here</h2>
      <p style={{ marginTop: 12, color: '#52525b', maxWidth: 760 }}>
        Next storefront package now follows the same live search and autocomplete contract as the
        Leptos storefront module.
      </p>

      <form
        onSubmit={(event) => {
          event.preventDefault();
          void loadResults(query, selectedPreset || null, catalogFilters);
        }}
        style={{ marginTop: 24, display: 'grid', gap: 12 }}
      >
        <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap' }}>
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search products and published content"
            style={{
              flex: '1 1 360px',
              minWidth: 0,
              borderRadius: 18,
              border: '1px solid #d4d4d8',
              padding: '14px 16px',
              fontSize: 15,
            }}
          />
          <button
            type="submit"
            style={{
              borderRadius: 18,
              border: 'none',
              background: '#18181b',
              color: '#fafafa',
              padding: '14px 18px',
              fontWeight: 600,
            }}
          >
            Search
          </button>
        </div>

        <div style={{ border: '1px solid #e4e4e7', borderRadius: 20, padding: 16 }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12 }}>
            <strong>Filter presets</strong>
            <span style={{ fontSize: 12, color: '#71717a' }}>
              {isLoadingPresets ? 'loading…' : 'surface defaults'}
            </span>
          </div>
          {presetsError ? (
            <p style={{ marginTop: 10, color: '#b91c1c' }}>{presetsError}</p>
          ) : presets.length === 0 ? (
            <p style={{ marginTop: 10, color: '#71717a' }}>No presets configured yet.</p>
          ) : (
            <div style={{ marginTop: 12, display: 'flex', flexWrap: 'wrap', gap: 8 }}>
              {presets.map((preset) => (
                <button
                  key={preset.key}
                  onClick={() => {
                    const nextPreset = selectedPreset === preset.key ? '' : preset.key;
                    setSelectedPreset(nextPreset);
                    void loadResults(query, nextPreset || null, catalogFilters);
                  }}
                  style={{
                    borderRadius: 999,
                    border: selectedPreset === preset.key ? '1px solid #0f766e' : '1px solid #d4d4d8',
                    background: selectedPreset === preset.key ? '#ccfbf1' : '#fff',
                    padding: '8px 12px',
                    fontSize: 12,
                    fontWeight: 600,
                  }}
                  type='button'
                >
                  {preset.label}
                </button>
              ))}
            </div>
          )}
        </div>

        <details style={{ border: '1px solid #e4e4e7', padding: 16 }}>
          <summary style={{ cursor: 'pointer', fontWeight: 600 }}>Catalog filters and sorting</summary>
          <div
            style={{
              marginTop: 14,
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
              gap: 12,
            }}
          >
            <CatalogField
              label='Channel ID'
              value={catalogFilters.channelId}
              onChange={(channelId) => setCatalogFilters((current) => ({ ...current, channelId }))}
            />
            <CatalogField
              label='Category IDs (CSV)'
              value={catalogFilters.categoryIds}
              onChange={(categoryIds) => setCatalogFilters((current) => ({ ...current, categoryIds }))}
              options={props.categoryOptions}
            />
            <CatalogField
              label='Attribute code'
              value={catalogFilters.attributeCode}
              onChange={(attributeCode) => setCatalogFilters((current) => ({ ...current, attributeCode }))}
              options={props.attributeOptions}
            />
            <CatalogField
              label='Attribute values (CSV)'
              value={catalogFilters.attributeValues}
              onChange={(attributeValues) => setCatalogFilters((current) => ({ ...current, attributeValues }))}
            />
            <CatalogField
              label='Minimum'
              value={catalogFilters.attributeMin}
              onChange={(attributeMin) => setCatalogFilters((current) => ({ ...current, attributeMin }))}
            />
            <CatalogField
              label='Maximum'
              value={catalogFilters.attributeMax}
              onChange={(attributeMax) => setCatalogFilters((current) => ({ ...current, attributeMax }))}
            />
            <CatalogField
              label='Sort attribute code'
              value={catalogFilters.sortAttributeCode}
              onChange={(sortAttributeCode) => setCatalogFilters((current) => ({ ...current, sortAttributeCode }))}
              options={props.attributeOptions}
            />
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 14 }}>
              <input
                type='checkbox'
                checked={catalogFilters.sortDesc}
                onChange={(event) =>
                  setCatalogFilters((current) => ({ ...current, sortDesc: event.target.checked }))
                }
              />
              Descending order
            </label>
          </div>
        </details>

        <div style={{ border: '1px solid #e4e4e7', borderRadius: 20, padding: 16 }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12 }}>
            <strong>Suggestions</strong>
            <span style={{ fontSize: 12, color: '#71717a' }}>
              {isLoadingSuggestions ? 'loading…' : 'autocomplete'}
            </span>
          </div>
          {suggestionsError ? (
            <p style={{ marginTop: 10, color: '#b91c1c' }}>{suggestionsError}</p>
          ) : suggestions.length === 0 ? (
            <p style={{ marginTop: 10, color: '#71717a' }}>
              Type at least 2 characters to load query and document suggestions.
            </p>
          ) : (
            <div style={{ marginTop: 12, display: 'grid', gap: 8 }}>
              {suggestions.map((suggestion) => (
                <button
                  key={`${suggestion.kind}:${suggestion.text}`}
                  onClick={() => {
                    if (suggestion.kind === 'document' && suggestion.url) {
                      window.location.href = suggestion.url;
                      return;
                    }
                    setQuery(suggestion.text);
                    void loadResults(suggestion.text, selectedPreset || null, catalogFilters);
                  }}
                  style={{
                    borderRadius: 16,
                    border: '1px solid #e4e4e7',
                    background: '#fff',
                    padding: 14,
                    textAlign: 'left',
                  }}
                  type="button"
                >
                  <div style={{ fontWeight: 600 }}>{suggestion.text}</div>
                  <div style={{ marginTop: 4, fontSize: 12, color: '#71717a' }}>
                    {suggestion.kind}
                    {suggestion.locale ? ` • ${suggestion.locale}` : ''}
                  </div>
                </button>
              ))}
            </div>
          )}
        </div>
      </form>

      <div style={{ marginTop: 24, border: '1px solid #e4e4e7', borderRadius: 20, padding: 20 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, flexWrap: 'wrap' }}>
          <strong>Results</strong>
          <span style={{ fontSize: 12, color: '#71717a' }}>
            {isLoadingResults
              ? 'loading…'
              : results
                ? `${results.total} hits via ${results.engine} (${results.rankingProfile})`
                : 'idle'}
          </span>
        </div>
        {searchError ? (
          <p style={{ marginTop: 12, color: '#b91c1c' }}>{searchError}</p>
        ) : !results ? (
          <p style={{ marginTop: 12, color: '#71717a' }}>
            Submit a query to preview the live storefront search contract in Next.
          </p>
        ) : (
          <div style={{ marginTop: 16, display: 'grid', gap: 14 }}>
            <div style={{ color: '#52525b' }}>
              {results.total} results in {results.tookMs} ms
              {results.presetKey ? ` • preset ${results.presetKey}` : ''}
            </div>
            {results.facets.length ? (
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                {results.facets.flatMap((facet) =>
                  facet.buckets.map((bucket) => (
                    <span
                      key={`${facet.name}:${bucket.value}`}
                      style={{ border: '1px solid #d4d4d8', padding: '6px 9px', fontSize: 12 }}
                    >
                      {bucket.label || bucket.value} ({bucket.count})
                    </span>
                  )),
                )}
              </div>
            ) : null}
            {results.items.map((item) => (
              <article
                key={item.id}
                style={{ border: '1px solid #e4e4e7', borderRadius: 18, padding: 16 }}
              >
                <div style={{ fontSize: 12, color: '#71717a', textTransform: 'uppercase' }}>
                  {item.entityType} • {item.sourceModule}
                </div>
                <h3 style={{ marginTop: 8, fontSize: 18 }}>{item.title}</h3>
                <p style={{ marginTop: 8, color: '#52525b' }}>
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
    <label style={{ display: 'grid', gap: 6, fontSize: 13 }}>
      <span>{props.label}</span>
      <input
        value={props.value}
        onChange={(event) => props.onChange(event.target.value)}
        list={props.options?.length ? listId : undefined}
        style={{ border: '1px solid #d4d4d8', padding: '10px 12px' }}
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
