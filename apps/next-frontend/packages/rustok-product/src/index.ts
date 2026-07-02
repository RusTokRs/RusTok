/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { storefrontGraphql } from "../../../src/shared/lib/graphql";

export type ProductCatalogSearchOption = {
  value: string;
  label: string;
};

export type ProductCatalogSearchOptions = {
  categoryOptions: ProductCatalogSearchOption[];
  attributeOptions: ProductCatalogSearchOption[];
};

export type ProductCatalogSearchOptionsRequest = {
  locale: string;
  token?: string | null;
  tenantSlug?: string | null;
  graphqlUrl?: string;
};

type StorefrontCatalogSearchOptionsResponse = {
  storefrontCatalogSearchOptions: ProductCatalogSearchOptions;
};

const STOREFRONT_CATALOG_SEARCH_OPTIONS_QUERY = `
  query StorefrontCatalogSearchOptions($locale: String!) {
    storefrontCatalogSearchOptions(locale: $locale) {
      categoryOptions { value label }
      attributeOptions { value label }
    }
  }
`;

export async function fetchCatalogSearchOptions(
  request: ProductCatalogSearchOptionsRequest,
): Promise<ProductCatalogSearchOptions> {
  const locale = request.locale.trim();
  if (!locale) {
    return { categoryOptions: [], attributeOptions: [] };
  }

  const response = await storefrontGraphql<
    StorefrontCatalogSearchOptionsResponse,
    { locale: string }
  >({
    query: STOREFRONT_CATALOG_SEARCH_OPTIONS_QUERY,
    variables: { locale },
    token: request.token ?? undefined,
    tenant: request.tenantSlug ?? undefined,
    baseUrl: request.graphqlUrl,
  });

  return (
    response.data?.storefrontCatalogSearchOptions ?? {
      categoryOptions: [],
      attributeOptions: [],
    }
  );
}
