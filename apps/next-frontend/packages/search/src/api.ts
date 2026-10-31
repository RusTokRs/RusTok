/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import type { storefrontGraphql } from '@/shared/lib/graphql';

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

export type SearchSuggestion = {
  text: string;
  kind: string;
  locale: string | null;
  url: string | null;
  score: number;
};

export type SearchResultItem = {
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

export type SearchFacetGroup = {
  name: string;
  buckets: Array<{
    value: string;
    label: string | null;
    count: number;
  }>;
};

export type SearchPreviewPayload = {
  queryLogId: string | null;
  presetKey: string | null;
  total: number;
  tookMs: number;
  engine: string;
  rankingProfile: string;
  items: SearchResultItem[];
  facets: SearchFacetGroup[];
};

export type SearchFilterPreset = {
  key: string;
  label: string;
  entityTypes: string[];
  sourceModules: string[];
  statuses: string[];
  rankingProfile: string | null;
};

export type StorefrontSearchResponse = {
  storefrontSearch: SearchPreviewPayload;
};

export type StorefrontSuggestionsResponse = {
  storefrontSearchSuggestions: SearchSuggestion[];
};

export type StorefrontFilterPresetsResponse = {
  storefrontSearchFilterPresets: SearchFilterPreset[];
};

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

export const STOREFRONT_SEARCH_QUERY = `
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

export const STOREFRONT_FILTER_PRESETS_QUERY = `
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

export const STOREFRONT_SUGGESTIONS_QUERY = `
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

export function parseCsv(value: string): string[] {
  return value
    .split(',')
    .map((segment) => segment.trim())
    .filter(Boolean);
}

export async function fetchStorefrontSearch(
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

  if ('errors' in response && response.errors?.length) {
    throw new Error(response.errors[0]?.message ?? 'Storefront search query failed');
  }

  const payload = response.data?.storefrontSearch;
  if (!payload) {
    throw new Error('storefrontSearch payload is missing');
  }

  return payload;
}

export async function fetchStorefrontFilterPresets(
  props: SearchStorefrontPageProps
): Promise<SearchFilterPreset[]> {
  const response = await props.graphql<StorefrontFilterPresetsResponse>({
    query: STOREFRONT_FILTER_PRESETS_QUERY,
    token: props.token ?? undefined,
    tenant: props.tenantSlug ?? undefined,
    baseUrl: props.graphqlUrl
  });

  if ('errors' in response && response.errors?.length) {
    throw new Error(response.errors[0]?.message ?? 'Storefront filter presets query failed');
  }

  return response.data?.storefrontSearchFilterPresets ?? [];
}

export async function fetchStorefrontSuggestions(
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

  if ('errors' in response && response.errors?.length) {
    throw new Error(response.errors[0]?.message ?? 'Storefront search suggestions query failed');
  }

  return response.data?.storefrontSearchSuggestions ?? [];
}
