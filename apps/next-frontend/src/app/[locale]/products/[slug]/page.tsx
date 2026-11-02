/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import type { Metadata } from "next";
import { locales } from "@/i18n-config";
import { cookies } from "next/headers";
import { ADMIN_TOKEN_KEY } from "@rustok/ui-auth/browser";
import { ProductRouteClient } from "@/features/product/components/product-route-client";
import { notFound } from "next/navigation";
import { storefrontGraphql } from "@/shared/lib/graphql";
import { getStorefrontTenantSlug } from "@/shared/api/modules";
import { buildSeoMetadata } from "@/shared/seo/metadata";
import { resolveSeoPageContextForRoute } from "@/shared/seo/runtime";
import {
  fetchStorefrontProduct,
  fetchStorefrontProductPricing,
  assertBuyerQuote,
  parseBuyerRoute,
  ProductBuyerError,
  type BuyerErrorCode,
  type BuyerRoute,
  type StorefrontProductPricingDetail,
} from "../../../../../packages/rustok-product/src";

interface ProductPageProps {
  params: Promise<{ locale: string; slug: string }>;
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}

export async function generateMetadata({
  params,
}: ProductPageProps): Promise<Metadata> {
  const { locale, slug } = await params;
  const tenantSlug = getStorefrontTenantSlug();
  const product = await fetchStorefrontProduct(
    storefrontGraphql,
    slug,
    locale,
    tenantSlug,
  );

  const path = `/products/${slug}`;
  const seoResolution = await resolveSeoPageContextForRoute({
    locale,
    route: path,
  });

  const translation =
    product?.translations.find((t) => t.locale === locale) ||
    product?.translations[0];

  const title = translation?.title || product?.handle || "Product Details";
  const description =
    translation?.description?.slice(0, 160) ||
    "Buy product online at RusToK Storefront.";

  return buildSeoMetadata({
    locale,
    title: `${title} | RusToK`,
    description,
    path,
    context: seoResolution.context,
  });
}

export default async function ProductDetailPage({
  params,
  searchParams,
}: ProductPageProps) {
  const { locale, slug } = await params;
  const query = searchParams ? await searchParams : {};
  const path = `/products/${slug}`;
  const seoResolution = await resolveSeoPageContextForRoute({
    locale,
    route: path,
  });

  const tenantSlug = getStorefrontTenantSlug();

  const product = await fetchStorefrontProduct(
    storefrontGraphql,
    slug,
    locale,
    tenantSlug,
  );

  if (!product) {
    notFound();
  }

  const token = (await cookies()).get(ADMIN_TOKEN_KEY)?.value ?? null;
  const buyerPath = `/${encodeURIComponent(locale)}/products/${encodeURIComponent(slug)}`;
  const originalSearch = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (Array.isArray(value))
      for (const entry of value) originalSearch.append(key, entry);
    else if (value !== undefined) originalSearch.set(key, value);
  }
  let buyerRoute: BuyerRoute | null = null;
  let buyerError: BuyerErrorCode | null = null;
  try {
    buyerRoute = parseBuyerRoute(product, query, locale);
  } catch (error) {
    buyerError = error instanceof ProductBuyerError ? error.code : "request";
  }

  let pricingData: StorefrontProductPricingDetail | null = null;
  if (buyerRoute?.selection.variant && buyerRoute.pricing.currencyCode) {
    const request = {
      ...buyerRoute.pricing,
      currencyCode: buyerRoute.pricing.currencyCode,
      quantity: buyerRoute.pricing.quantity ?? 1,
    };
    try {
      pricingData = await fetchStorefrontProductPricing(
        storefrontGraphql,
        slug,
        locale,
        {
          currencyCode: request.currencyCode,
          regionId: request.regionId ?? undefined,
          priceListId: request.priceListId ?? undefined,
          channelId: request.channelId ?? undefined,
          channelSlug: request.channelSlug ?? undefined,
          quantity: request.quantity,
        },
        tenantSlug,
        token,
      );
      if (pricingData) assertBuyerQuote(product, pricingData, request);
    } catch {
      // Keep catalog content, but never seed the buyer panel with a rejected/partial quote.
      pricingData = null;
    }
  }

  return (
    <main className="min-h-screen bg-background">
      <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8 py-10">
        <ProductRouteClient
          key={`${tenantSlug}:${locale}:${product.id}:${originalSearch.toString()}`}
          product={product}
          handle={slug}
          locale={locale}
          tenantSlug={tenantSlug}
          path={buyerPath}
          localePaths={Object.fromEntries(
            locales.map((supported) => [
              supported,
              `/${encodeURIComponent(supported)}/products/${encodeURIComponent(slug)}`,
            ]),
          )}
          initialSearch={originalSearch.toString()}
          initialRoute={buyerRoute}
          initialError={buyerError}
          initialQuote={pricingData}
        />
      </div>
    </main>
  );
}
