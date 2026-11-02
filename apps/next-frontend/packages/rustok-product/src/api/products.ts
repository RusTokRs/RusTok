/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import {
  assertPricingGraphqlSuccess,
  assertPricingQuoteResponse,
  normalizePricingQuery,
} from "@rustok/pricing-frontend";
import type { storefrontGraphql } from "@/shared/lib/graphql";
import type {
  ProductCatalogFacet,
  ProductCatalogSearchOptions,
  StorefrontCatalogFilter,
  StorefrontProductDetail,
  StorefrontProductListResponse,
  StorefrontProductPricingDetail,
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
        primaryImage {
          mediaId
          url
          altText
          position
        }
        priceFrom {
          currencyCode
          amount
          compareAtAmount
          onSale
        }
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
      images {
        mediaId
        url
        altText
        position
      }
      translations {
        locale
        title
        handle
        description
      }
      variantAxes {
        id attributeId code name position
        allowedValues { optionId value position }
      }
      variants {
        id
        title
        sku
        combinationIdentity
        axisValues { attributeId optionId code label }
        inventoryPolicy
        inventoryQuantity
        inStock
        prices {
          currencyCode
          amount
          compareAtAmount
          onSale
        }
      }
      attributes {
        code
        label
        valueType
        isLocalized
        values {
          text
        }
      }
    }
  }
`;

const STOREFRONT_PRICING_PRODUCT_QUERY = `
  query StorefrontProductPricing(
    $locale: String,
    $handle: String!,
    $currencyCode: String,
    $regionId: UUID,
    $priceListId: UUID,
    $channelId: UUID,
    $channelSlug: String,
    $quantity: Int
  ) {
    storefrontPricingProduct(
      locale: $locale,
      handle: $handle,
      currencyCode: $currencyCode,
      regionId: $regionId,
      priceListId: $priceListId,
      channelId: $channelId,
      channelSlug: $channelSlug,
      quantity: $quantity
    ) {
      id
      resolutionContext { currencyCode regionId priceListId channelId channelSlug quantity }
      resolvedAt
      variants {
        id
        effectivePrice {
          source { priceId kind ruleKind adjustmentPercent }
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

/**
 * Bucket counts for typed EAV facets under the current storefront filter set.
 *
 * Facets are aggregated over the whole filtered catalog, so the request deliberately carries no
 * pagination: the owner counts every matching product, not one page.
 */
const STOREFRONT_CATALOG_FACETS_QUERY = `
  query StorefrontCatalogFacets(
    $locale: String,
    $filter: StorefrontProductCatalogFilter,
    $facetCodes: [String!]!
  ) {
    storefrontProductCatalogFacets(
      locale: $locale,
      filter: $filter,
      facetCodes: $facetCodes
    ) {
      code
      label
      valueType
      isLocalized
      isEnumerable
      isTruncated
      totalProducts
      values {
        value
        label
        count
      }
    }
  }
`;

type CatalogFilterVariables = {
  search?: string;
  categoryId?: string;
  sortBy?: string;
  sortDirection?: string;
  attributeFilters?: string[];
  currencyCode?: string;
  page?: number;
  perPage?: number;
};

function toCatalogFilterVariables(
  filter: StorefrontCatalogFilter | undefined,
  paginate: boolean,
): CatalogFilterVariables | undefined {
  if (!filter) return undefined;
  const variables: CatalogFilterVariables = {
    search: filter.search?.trim() || undefined,
    categoryId: filter.categoryId?.trim() || undefined,
    sortBy: filter.sortBy || undefined,
    sortDirection: filter.sortDirection || undefined,
    attributeFilters: filter.attributeFilters?.length
      ? filter.attributeFilters
      : undefined,
    currencyCode: filter.currencyCode?.trim().toUpperCase() || undefined,
    page: filter.page ?? 1,
    perPage: filter.perPage ?? 12,
  };
  if (!paginate) {
    delete variables.page;
    delete variables.perPage;
  }
  return variables;
}

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

  const response = await graphql<
    {
      storefrontCatalogSearchOptions: ProductCatalogSearchOptions;
    },
    { locale: string }
  >({
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
  const response = await graphql<
    {
      storefrontProductCatalog: StorefrontProductListResponse;
    },
    {
      locale?: string;
      filter?: CatalogFilterVariables;
    }
  >({
    query: STOREFRONT_PRODUCTS_QUERY,
    variables: {
      locale: locale.trim() || undefined,
      filter: toCatalogFilterVariables(filter, true),
    },
    tenant: tenantSlug ?? undefined,
  });

  if (response.errors?.length || !response.data?.storefrontProductCatalog)
    throw new Error("Product catalog response is unavailable");
  return response.data.storefrontProductCatalog;
}

/**
 * Loads the owner-counted facets for the requested attribute codes.
 *
 * The caller derives the codes from the same catalog search options it renders as filter inputs
 * (`buildCatalogFacetCodes`); the owner answers in that order and ignores unknown codes.
 */
export async function fetchStorefrontCatalogFacets(
  graphql: ProductGraphqlExecutor,
  locale: string,
  filter: StorefrontCatalogFilter | undefined,
  facetCodes: readonly string[],
  tenantSlug?: string | null,
): Promise<ProductCatalogFacet[]> {
  const codes = facetCodes
    .map((code) => code.trim())
    .filter((code) => code.length > 0);
  if (codes.length === 0) return [];

  const response = await graphql<
    {
      storefrontProductCatalogFacets: ProductCatalogFacet[];
    },
    {
      locale?: string;
      filter?: CatalogFilterVariables;
      facetCodes: string[];
    }
  >({
    query: STOREFRONT_CATALOG_FACETS_QUERY,
    variables: {
      locale: locale.trim() || undefined,
      filter: toCatalogFilterVariables(filter, false),
      facetCodes: codes,
    },
    tenant: tenantSlug ?? undefined,
  });

  return response.data?.storefrontProductCatalogFacets ?? [];
}

export async function fetchStorefrontProduct(
  graphql: ProductGraphqlExecutor,
  handle: string,
  locale?: string,
  tenantSlug?: string | null,
): Promise<StorefrontProductDetail | null> {
  const cleanHandle = handle.trim();
  if (!cleanHandle) return null;

  const response = await graphql<
    {
      storefrontProduct: StorefrontProductDetail | null;
    },
    {
      locale?: string;
      handle: string;
    }
  >({
    query: STOREFRONT_PRODUCT_QUERY,
    variables: {
      handle: cleanHandle,
      locale: locale?.trim() || undefined,
    },
    tenant: tenantSlug ?? undefined,
  });

  // A failed/partial catalog read is not an authoritative "not found" or a usable tuple matrix.
  if (
    response.errors?.length ||
    !response.data ||
    response.data.storefrontProduct === undefined
  )
    throw new Error("Product catalog response is unavailable");
  return response.data.storefrontProduct;
}

export type StorefrontPricingOptions = {
  currencyCode?: string;
  regionId?: string;
  priceListId?: string;
  channelId?: string;
  channelSlug?: string;
  quantity?: number;
};

export async function fetchStorefrontProductPricing(
  graphql: ProductGraphqlExecutor,
  handle: string,
  locale?: string,
  currencyOrOptions?: string | StorefrontPricingOptions,
  tenantSlug?: string | null,
  token?: string | null,
): Promise<StorefrontProductPricingDetail | null> {
  const cleanHandle = handle.trim();
  if (!cleanHandle) return null;

  const options: StorefrontPricingOptions =
    typeof currencyOrOptions === "string"
      ? { currencyCode: currencyOrOptions }
      : (currencyOrOptions ?? {});

  const request = normalizePricingQuery({ ...options, locale });
  if (!request.currencyCode)
    throw new Error("A buyer quote requires a currency");

  const response = await graphql<
    {
      storefrontPricingProduct: StorefrontProductPricingDetail | null;
    },
    {
      locale?: string;
      handle: string;
      currencyCode?: string;
      regionId?: string;
      priceListId?: string;
      channelId?: string;
      channelSlug?: string;
      quantity?: number;
    }
  >({
    query: STOREFRONT_PRICING_PRODUCT_QUERY,
    variables: {
      handle: cleanHandle,
      locale: request.locale ?? undefined,
      currencyCode: request.currencyCode,
      regionId: request.regionId ?? undefined,
      priceListId: request.priceListId ?? undefined,
      channelId: request.channelId ?? undefined,
      channelSlug: request.channelSlug ?? undefined,
      quantity: request.quantity ?? undefined,
    },
    tenant: tenantSlug ?? undefined,
    token: token ?? undefined,
  });

  assertPricingGraphqlSuccess(response);
  const pricing = response.data?.storefrontPricingProduct;
  if (!response.data || pricing === undefined)
    throw new Error("Pricing response is unavailable");
  if (!pricing) return pricing;
  // A requested quote with missing/mismatched metadata is unavailable, never raw-price inspection.
  assertPricingQuoteResponse(pricing, request);
  return pricing;
}
