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
import { fetchCatalogSearchOptions } from '@rustok/product-frontend';
import {
  SearchStorefrontPage,
  type SearchCatalogFilterOption
} from '@rustok/search-frontend';

interface SearchPageProps {
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
  const path = '/search';
  const seoResolution = await resolveSeoPageContextForRoute({
    locale,
    route: path,
  });

  return buildSeoMetadata({
    locale,
    title: isRu ? 'Поиск по каталогу | RusToK' : 'Catalog Search | RusToK',
    description: isRu
      ? 'Мгновенный поиск товаров, категорий и предложений с фасетной фильтрацией'
      : 'Instant catalog search for products, categories, and offers with faceted filtering',
    path,
    context: seoResolution.context,
  });
}

export default async function SearchRoutePage({
  params,
  searchParams,
}: SearchPageProps) {
  const { locale } = await params;
  const query = await searchParams;
  const tenantSlug = getStorefrontTenantSlug();

  const initialQuery =
    typeof query.q === 'string'
      ? query.q
      : typeof query.search === 'string'
      ? query.search
      : '';

  let categoryOptions: SearchCatalogFilterOption[] = [];
  let attributeOptions: SearchCatalogFilterOption[] = [];

  try {
    const opts = await fetchCatalogSearchOptions(
      storefrontGraphql,
      locale,
      tenantSlug
    );
    categoryOptions = opts.categoryOptions || [];
    attributeOptions = opts.attributeOptions || [];
  } catch {
    categoryOptions = [];
    attributeOptions = [];
  }

  return (
    <main className='min-h-screen bg-background'>
      <div className='mx-auto max-w-7xl px-4 sm:px-6 lg:px-8 py-10 space-y-8'>
        <div className='space-y-2 border-b border-border pb-6'>
          <h1 className='text-3xl font-bold tracking-tight text-foreground sm:text-4xl'>
            {locale === 'ru' ? 'Поиск по каталогу' : 'Catalog Search'}
          </h1>
          <p className='text-sm text-muted-foreground max-w-2xl'>
            {locale === 'ru'
              ? 'Мгновенный поиск товаров, подсказки на лету, фасетная фильтрация и ранжирование.'
              : 'Instant search across inventory, typeahead suggestions, faceted filters, and ranking.'}
          </p>
        </div>

        <SearchStorefrontPage
          graphql={storefrontGraphql}
          tenantSlug={tenantSlug}
          locale={locale}
          initialQuery={initialQuery}
          categoryOptions={categoryOptions}
          attributeOptions={attributeOptions}
        />
      </div>
    </main>
  );
}
