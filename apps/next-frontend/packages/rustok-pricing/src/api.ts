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
import type {
  PricingChannelOption,
  PricingPriceListOption,
  PricingProductDetail,
  PricingProductList,
  StorefrontPricingData,
  StorefrontPricingQuery
} from './types';

type GraphqlExecutor = typeof storefrontGraphql;

export const STOREFRONT_PRODUCTS_QUERY = `
  query StorefrontCommerceProducts(
    $locale: String
    $channelId: UUID
    $channelSlug: String
  ) {
    storefrontProducts(locale: $locale) {
      total
      page
      perPage
      hasNext
      items {
        id
        title
        handle
        sellerId
        vendor
        productType
        createdAt
        publishedAt
      }
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
  tenantSlug?: string | null
): Promise<StorefrontPricingData> {
  const [productsRes, productRes] = await Promise.all([
    graphql<{
      storefrontProducts?: {
        items: Array<{
          id: string;
          title: string;
          handle: string;
          sellerId?: string | null;
          vendor?: string | null;
          productType?: string | null;
          createdAt: string;
          publishedAt?: string | null;
        }>;
        total: number;
        page: number;
        perPage: number;
        hasNext: boolean;
      };
      storefrontPricingChannels?: PricingChannelOption[];
      storefrontActivePriceLists?: PricingPriceListOption[];
    }>({
      query: STOREFRONT_PRODUCTS_QUERY,
      variables: {
        locale: query.locale ?? undefined,
        channelId: query.channelId ?? undefined,
        channelSlug: query.channelSlug ?? undefined
      },
      token: token ?? undefined,
      tenant: tenantSlug ?? undefined
    }),
    query.selectedHandle
      ? graphql<{
          storefrontPricingProduct?: PricingProductDetail | null;
        }>({
          query: STOREFRONT_PRODUCT_QUERY,
          variables: {
            locale: query.locale ?? undefined,
            handle: query.selectedHandle,
            currencyCode: query.currencyCode ?? undefined,
            regionId: query.regionId ?? undefined,
            priceListId: query.priceListId ?? undefined,
            channelId: query.channelId ?? undefined,
            channelSlug: query.channelSlug ?? undefined,
            quantity: query.quantity ?? undefined
          },
          token: token ?? undefined,
          tenant: tenantSlug ?? undefined
        })
      : Promise.resolve({ data: { storefrontPricingProduct: null } })
  ]);

  const rawProducts = productsRes.data?.storefrontProducts;
  const items = (rawProducts?.items ?? []).map((item) => ({
    ...item,
    variantCount: 1,
    saleVariantCount: 0,
    currencies: query.currencyCode ? [query.currencyCode] : ['USD']
  }));

  const products: PricingProductList = {
    items,
    total: rawProducts?.total ?? items.length,
    page: rawProducts?.page ?? 1,
    perPage: rawProducts?.perPage ?? 20,
    hasNext: rawProducts?.hasNext ?? false
  };

  return {
    products,
    selectedProduct: productRes.data?.storefrontPricingProduct ?? null,
    selectedHandle: query.selectedHandle ?? null,
    resolutionContext: {
      currencyCode: query.currencyCode ?? 'USD',
      regionId: query.regionId ?? null,
      priceListId: query.priceListId ?? null,
      channelId: query.channelId ?? null,
      channelSlug: query.channelSlug ?? null,
      quantity: query.quantity ?? 1
    },
    availableChannels: productsRes.data?.storefrontPricingChannels ?? [],
    activePriceLists: productsRes.data?.storefrontActivePriceLists ?? []
  };
}
