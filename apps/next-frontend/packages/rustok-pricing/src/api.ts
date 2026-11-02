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
import {
  assertPricingGraphqlSuccess,
  assertPricingQuoteResponse,
  normalizePricingQuery,
} from "./core";
import type {
  PricingChannelOption,
  PricingPriceListOption,
  PricingProductDetail,
  PricingProductList,
  StorefrontPricingData,
  StorefrontPricingQuery,
} from "./types";

type GraphqlExecutor = typeof storefrontGraphql;

export const STOREFRONT_PRODUCTS_QUERY = `
  query StorefrontCommerceProducts(
    $locale: String
    $channelId: UUID
    $channelSlug: String
  ) {
    storefrontPricingCatalog(locale: $locale, channelId: $channelId, channelSlug: $channelSlug, page: 1, perPage: 8) {
      total page perPage hasNext
      items { id title handle sellerId vendor productType createdAt publishedAt variantCount saleVariantCount currencies }
    }
    storefrontPricingChannels {
      id
      slug
      name
      isActive
      isDefault
      status
    }
    storefrontActivePriceLists(channelId: $channelId, channelSlug: $channelSlug) {
      id
      name
      listType
      channelId
      channelSlug
      ruleKind
      adjustmentPercent
    }
  }
`;

export const STOREFRONT_PRODUCT_QUERY = `
  query StorefrontCommerceProduct(
    $locale: String
    $handle: String!
    $currencyCode: String
    $regionId: UUID
    $priceListId: UUID
    $channelId: UUID
    $channelSlug: String
    $quantity: Int
  ) {
    storefrontPricingProduct(
      locale: $locale
      handle: $handle
      currencyCode: $currencyCode
      regionId: $regionId
      priceListId: $priceListId
      channelId: $channelId
      channelSlug: $channelSlug
      quantity: $quantity
    ) {
      resolutionContext { currencyCode regionId priceListId channelId channelSlug quantity }
      resolvedAt
      id
      status
      sellerId
      vendor
      productType
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
        prices {
          currencyCode
          amount
          compareAtAmount
          discountPercent
          onSale
        }
        effectivePrice {
          source { priceId kind ruleKind adjustmentPercent }
          currencyCode
          amount
          compareAtAmount
          discountPercent
          onSale
          regionId
          priceListId
          channelId
          channelSlug
          minQuantity
          maxQuantity
        }
      }
    }
  }
`;

export async function fetchStorefrontPricing(
  graphql: GraphqlExecutor,
  query: StorefrontPricingQuery,
  token?: string | null,
  tenantSlug?: string | null,
): Promise<StorefrontPricingData> {
  query = normalizePricingQuery(query);
  const productsRes = await graphql<{
    storefrontPricingCatalog: PricingProductList;
    storefrontPricingChannels: PricingChannelOption[];
    storefrontActivePriceLists: PricingPriceListOption[];
  }>({
    query: STOREFRONT_PRODUCTS_QUERY,
    variables: {
      locale: query.locale ?? undefined,
      channelId: query.channelId ?? undefined,
      channelSlug: query.channelSlug ?? undefined,
    },
    token: token ?? undefined,
    tenant: tenantSlug ?? undefined,
  });
  assertPricingGraphqlSuccess(productsRes);
  const products = productsRes.data?.storefrontPricingCatalog;
  if (!products) throw new Error("Pricing catalog response is unavailable");
  const availableChannels = productsRes.data?.storefrontPricingChannels;
  const activePriceLists = productsRes.data?.storefrontActivePriceLists;
  if (!availableChannels || !activePriceLists)
    throw new Error("Pricing discovery response is unavailable");
  const selectedHandle =
    query.selectedHandle?.trim() || products.items[0]?.handle || null;
  let selectedProduct: PricingProductDetail | null = null;
  if (selectedHandle) {
    const productRes = await graphql<{
      storefrontPricingProduct: PricingProductDetail | null;
    }>({
      query: STOREFRONT_PRODUCT_QUERY,
      variables: {
        locale: query.locale ?? undefined,
        handle: selectedHandle,
        currencyCode: query.currencyCode || undefined,
        regionId: query.regionId ?? undefined,
        priceListId: query.priceListId ?? undefined,
        channelId: query.channelId ?? undefined,
        channelSlug: query.channelSlug ?? undefined,
        quantity: query.quantity ?? undefined,
      },
      token: token ?? undefined,
      tenant: tenantSlug ?? undefined,
    });
    assertPricingGraphqlSuccess(productRes);
    if (!productRes.data || !("storefrontPricingProduct" in productRes.data)) {
      throw new Error("Pricing product response is unavailable");
    }
    selectedProduct = productRes.data.storefrontPricingProduct;
    if (selectedProduct) assertPricingQuoteResponse(selectedProduct, query);
  }
  return {
    products,
    selectedProduct,
    selectedHandle,
    resolutionContext: selectedProduct?.resolutionContext ?? null,
    availableChannels,
    activePriceLists,
  };
}
