/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import type { storefrontGraphql } from "@/shared/lib/graphql";
import type {
  ProductCatalogSearchOptions,
  StorefrontCatalogFilter,
  StorefrontProductDetail,
  StorefrontProductListResponse,
  StorefrontProductVariant,
} from "./types";

export type ProductGraphqlExecutor = typeof storefrontGraphql;

const STOREFRONT_PRODUCTS_QUERY = `
  query StorefrontProductCatalog($locale: String, $filter: StorefrontProductCatalogFilter) {
    storefrontProductCatalog(locale: $locale, filter: $filter) {
      total
      page
      perPage
      hasNext
      items {
        id
        status
        title
        handle
        sellerId
        vendor
        productType
        tags
        createdAt
        publishedAt
      }
    }
  }
`;

const STOREFRONT_PRODUCT_QUERY = `
  query StorefrontCommerceProduct($locale: String, $handle: String!) {
    storefrontProduct(locale: $locale, handle: $handle) {
      id
      status
      sellerId
      vendor
      productType
      tags
      publishedAt
      translations {
        locale
        title
        handle
        description
      }
      variants {
        id
        title
        sku
        inventoryQuantity
        inStock
        prices {
          currencyCode
          amount
          compareAtAmount
          onSale
        }
      }
    }
  }
`;

const STOREFRONT_PRICING_PRODUCT_QUERY = `
  query StorefrontProductPricing(
    $locale: String,
    $handle: String!,
    $currencyCode: String
  ) {
    storefrontPricingProduct(
      locale: $locale,
      handle: $handle,
      currencyCode: $currencyCode
    ) {
      variants {
        id
        title
        sku
        prices {
          currencyCode
          amount
          compareAtAmount
          discountPercent
          onSale
        }
        effectivePrice {
          currencyCode
          amount
          compareAtAmount
          discountPercent
          onSale
          priceListId
          channelId
          channelSlug
        }
      }
    }
  }
`;

const STOREFRONT_CATALOG_SEARCH_OPTIONS_QUERY = `
  query StorefrontCatalogSearchOptions($locale: String!) {
    storefrontCatalogSearchOptions(locale: $locale) {
      categoryOptions { value label }
      attributeOptions { value label }
    }
  }
`;

export async function fetchCatalogSearchOptions(
  requestOrGraphql:
    | {
        graphql: ProductGraphqlExecutor;
        locale: string;
        token?: string | null;
        tenantSlug?: string | null;
        graphqlUrl?: string;
      }
    | ProductGraphqlExecutor,
  localeParam?: string,
  tenantSlugParam?: string | null,
): Promise<ProductCatalogSearchOptions> {
  let graphql: ProductGraphqlExecutor;
  let locale: string;
  let tenantSlug: string | null | undefined;
  let token: string | null | undefined;
  let baseUrl: string | undefined;

  if (typeof requestOrGraphql === "function") {
    graphql = requestOrGraphql;
    locale = localeParam || "";
    tenantSlug = tenantSlugParam;
  } else {
    graphql = requestOrGraphql.graphql;
    locale = requestOrGraphql.locale;
    tenantSlug = requestOrGraphql.tenantSlug;
    token = requestOrGraphql.token;
    baseUrl = requestOrGraphql.graphqlUrl;
  }

  const normLocale = locale.trim();
  if (!normLocale) {
    return { categoryOptions: [], attributeOptions: [] };
  }

  const response = await graphql<{
    storefrontCatalogSearchOptions: ProductCatalogSearchOptions;
  }, { locale: string }>({
    query: STOREFRONT_CATALOG_SEARCH_OPTIONS_QUERY,
    variables: { locale: normLocale },
    tenant: tenantSlug ?? undefined,
    token: token ?? undefined,
    baseUrl,
  });

  return (
    response.data?.storefrontCatalogSearchOptions ?? {
      categoryOptions: [],
      attributeOptions: [],
    }
  );
}

export async function fetchStorefrontProducts(
  graphql: ProductGraphqlExecutor,
  locale: string,
  filter?: StorefrontCatalogFilter,
  tenantSlug?: string | null,
): Promise<StorefrontProductListResponse> {
  const response = await graphql<{
    storefrontProductCatalog: StorefrontProductListResponse;
  }, {
    locale?: string;
    filter?: {
      search?: string;
      categoryId?: string;
      sortBy?: string;
      sortDirection?: string;
      attributeFilters?: string[];
      page?: number;
      perPage?: number;
    };
  }>({
    query: STOREFRONT_PRODUCTS_QUERY,
    variables: {
      locale: locale.trim() || undefined,
      filter: filter
        ? {
            search: filter.search?.trim() || undefined,
            categoryId: filter.categoryId?.trim() || undefined,
            sortBy: filter.sortBy || undefined,
            sortDirection: filter.sortDirection || undefined,
            attributeFilters: filter.attributeFilters?.length
              ? filter.attributeFilters
              : undefined,
            page: filter.page ?? 1,
            perPage: filter.perPage ?? 12,
          }
        : undefined,
    },
    tenant: tenantSlug ?? undefined,
  });

  return (
    response.data?.storefrontProductCatalog ?? {
      total: 0,
      page: 1,
      perPage: 12,
      hasNext: false,
      items: [],
    }
  );
}

export async function fetchStorefrontProduct(
  graphql: ProductGraphqlExecutor,
  handle: string,
  locale?: string,
  tenantSlug?: string | null,
): Promise<StorefrontProductDetail | null> {
  const cleanHandle = handle.trim();
  if (!cleanHandle) return null;

  const response = await graphql<{
    storefrontProduct: StorefrontProductDetail | null;
  }, {
    locale?: string;
    handle: string;
  }>({
    query: STOREFRONT_PRODUCT_QUERY,
    variables: {
      handle: cleanHandle,
      locale: locale?.trim() || undefined,
    },
    tenant: tenantSlug ?? undefined,
  });

  return response.data?.storefrontProduct ?? null;
}

export async function fetchStorefrontProductPricing(
  graphql: ProductGraphqlExecutor,
  handle: string,
  locale?: string,
  currencyCode?: string,
  tenantSlug?: string | null,
): Promise<{ variants: StorefrontProductVariant[] } | null> {
  const cleanHandle = handle.trim();
  if (!cleanHandle) return null;

  try {
    const response = await graphql<{
      storefrontPricingProduct: { variants: StorefrontProductVariant[] } | null;
    }, {
      locale?: string;
      handle: string;
      currencyCode?: string;
    }>({
      query: STOREFRONT_PRICING_PRODUCT_QUERY,
      variables: {
        handle: cleanHandle,
        locale: locale?.trim() || undefined,
        currencyCode: currencyCode?.trim() || undefined,
      },
      tenant: tenantSlug ?? undefined,
    });

    return response.data?.storefrontPricingProduct ?? null;
  } catch {
    return null;
  }
}
