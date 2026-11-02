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
import { storefrontGraphql } from "@/shared/lib/graphql";
import { getStorefrontTenantSlug } from "@/shared/api/modules";
import { buildSeoMetadata } from "@/shared/seo/metadata";
import { resolveSeoPageContextForRoute } from "@/shared/seo/runtime";
import {
  PricingRequestError,
  fetchStorefrontPricing,
  parsePricingRouteQuery,
  pricingReadKey,
  type StorefrontPricingQuery,
} from "@rustok/pricing-frontend";
import { PricingRouteClient } from "@/features/pricing/components/pricing-route-client";

interface PricingPageProps {
  params: Promise<{ locale: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}

export async function generateMetadata({
  params,
}: {
  params: Promise<{ locale: string }>;
}): Promise<Metadata> {
  const { locale } = await params;
  const isRu = locale === "ru";
  const path = "/pricing";
  const seoResolution = await resolveSeoPageContextForRoute({
    locale,
    route: path,
  });

  return buildSeoMetadata({
    locale,
    title: isRu
      ? "Каталог цен и скидок | RusToK"
      : "Pricing & Discounts Atlas | RusToK",
    description: isRu
      ? "Публичный атлас цен, валютного покрытия и активных скидок RusToK"
      : "Public atlas of prices, currencies, and discounts across channels",
    path,
    context: seoResolution.context,
  });
}

export default async function PricingRoutePage({
  params,
  searchParams,
}: PricingPageProps) {
  const { locale } = await params;
  const query = await searchParams;
  const tenantSlug = getStorefrontTenantSlug();
  const resetHref = `/${encodeURIComponent(locale)}/pricing`;

  let pricingQuery: StorefrontPricingQuery;
  try {
    pricingQuery = parsePricingRouteQuery(query, locale);
  } catch {
    return (
      <main className="mx-auto max-w-7xl px-4 py-8 sm:px-6 lg:px-8">
        <PricingRequestError resetHref={resetHref} />
      </main>
    );
  }
  const initialData = await fetchStorefrontPricing(
    storefrontGraphql,
    pricingQuery,
    null,
    tenantSlug,
  ).catch(() => null);

  return (
    <main className="mx-auto max-w-7xl px-4 py-8 sm:px-6 lg:px-8">
      <PricingRouteClient
        key={pricingReadKey(pricingQuery, tenantSlug, null)}
        initialData={initialData}
        initialQuery={pricingQuery}
        resetHref={resetHref}
        tenantSlug={tenantSlug}
        locale={locale}
      />
    </main>
  );
}
