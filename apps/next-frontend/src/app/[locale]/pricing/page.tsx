/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import type { Metadata } from 'next';
import { storefrontGraphql } from '@/shared/lib/graphql';
import { getStorefrontTenantSlug } from '@/shared/api/modules';
import { buildSeoMetadata } from '@/shared/seo/metadata';
import { resolveSeoPageContextForRoute } from '@/shared/seo/runtime';
import {
  PricingView,
  fetchStorefrontPricing
} from '@rustok/pricing-frontend';

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
  const isRu = locale === 'ru';
  const path = '/pricing';
  const seoResolution = await resolveSeoPageContextForRoute({
    locale,
    route: path,
  });

  return buildSeoMetadata({
    locale,
    title: isRu ? 'Каталог цен и скидок | RusToK' : 'Pricing & Discounts Atlas | RusToK',
    description: isRu
      ? 'Публичный атлас цен, валютного покрытия и активных скидок RusToK'
      : 'Public atlas of prices, currencies, and discounts across channels',
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

  const selectedHandle =
    typeof query.handle === 'string' ? query.handle : null;
  const currencyCode =
    typeof query.currency === 'string' ? query.currency : null;
  const channelId =
    typeof query.channel_id === 'string' ? query.channel_id : null;
  const channelSlug =
    typeof query.channel_slug === 'string' ? query.channel_slug : null;
  const priceListId =
    typeof query.price_list_id === 'string' ? query.price_list_id : null;
  const quantity =
    typeof query.quantity === 'string'
      ? parseInt(query.quantity, 10) || 1
      : 1;

  const initialData = await fetchStorefrontPricing(
    storefrontGraphql,
    {
      selectedHandle,
      locale,
      currencyCode,
      channelId,
      channelSlug,
      priceListId,
      quantity
    },
    null,
    tenantSlug
  ).catch(() => null);

  return (
    <main className='mx-auto max-w-7xl px-4 py-8 sm:px-6 lg:px-8'>
      <PricingView
        initialData={initialData}
        graphql={storefrontGraphql}
        tenantSlug={tenantSlug}
        locale={locale}
      />
    </main>
  );
}
