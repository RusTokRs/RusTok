/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

import type { Metadata } from 'next';
import Link from 'next/link';
import { storefrontGraphql } from '@/shared/lib/graphql';
import { getStorefrontTenantSlug } from '@/shared/api/modules';
import { Tag, ExternalLink, ArrowRight } from 'lucide-react';

interface BrandsPageProps {
  params: Promise<{ locale: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}

interface BrandItem {
  id: string;
  slug: string;
  name: string;
  description?: string | null;
  websiteUrl?: string | null;
  logoUrl?: string | null;
  productsCount: number;
}

const STOREFRONT_BRANDS_QUERY = `
query StorefrontBrands($locale: String, $categoryId: UUID) {
  brands(filter: { isActive: true, categoryId: $categoryId }, locale: $locale, perPage: 100) {
    items {
      id
      slug
      name
      description
      websiteUrl
      logoUrl
      productsCount
    }
  }
}`;

export async function generateMetadata({
  params
}: {
  params: Promise<{ locale: string }>;
}): Promise<Metadata> {
  const { locale } = await params;
  const isRu = locale === 'ru';

  return {
    title: isRu ? 'Бренды | RusToK' : 'Brands | RusToK',
    description: isRu
      ? 'Каталог официальных брендов и производителей нашего маркетплейса'
      : 'Explore official brands and manufacturers in our catalog'
  };
}

export default async function BrandsPage({
  params,
  searchParams
}: BrandsPageProps) {
  const { locale } = await params;
  const resolvedSearchParams = await searchParams;
  const isRu = locale === 'ru';
  const tenantSlug = await getStorefrontTenantSlug();

  const categoryId =
    typeof resolvedSearchParams.category === 'string'
      ? resolvedSearchParams.category
      : null;

  let brands: BrandItem[] = [];

  try {
    const res = await storefrontGraphql<{ brands: { items: BrandItem[] } }>({
      query: STOREFRONT_BRANDS_QUERY,
      variables: {
        locale,
        categoryId
      },
      tenant: tenantSlug ?? undefined
    });
    brands = res.data?.brands?.items ?? [];
  } catch {
    brands = [];
  }

  return (
    <div className='container mx-auto px-4 py-8 max-w-6xl'>
      <div className='mb-8 border-b pb-6'>
        <div className='flex items-center gap-2 text-xs text-muted-foreground mb-2'>
          <Link href={`/${locale}`} className='hover:underline'>
            {isRu ? 'Главная' : 'Home'}
          </Link>
          <span>/</span>
          <span className='text-foreground'>{isRu ? 'Бренды' : 'Brands'}</span>
        </div>
        <h1 className='text-3xl font-bold tracking-tight text-foreground'>
          {isRu ? 'Каталог брендов' : 'Brand Directory'}
        </h1>
        <p className='text-muted-foreground mt-2 text-sm max-w-2xl'>
          {isRu
            ? 'Все официальные бренды, производители и коллаборации, представленные на нашей платформе.'
            : 'All official brands, manufacturers, and collaboration partners available across our catalog.'}
        </p>
      </div>

      {brands.length === 0 ? (
        <div className='py-16 text-center text-muted-foreground'>
          <Tag className='h-12 w-12 mx-auto mb-3 opacity-40' />
          <p className='text-base font-medium'>
            {isRu ? 'Бренды не найдены' : 'No brands found'}
          </p>
          <p className='text-sm mt-1'>
            {isRu
              ? 'В этой категории пока нет зарегистрированных брендов.'
              : 'There are no registered brands matching the selection.'}
          </p>
        </div>
      ) : (
        <div className='grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-6'>
          {brands.map((brand) => (
            <Link
              key={brand.id}
              href={`/${locale}/brands/${brand.slug}`}
              className='group flex flex-col justify-between rounded-xl border border-border bg-card p-6 shadow-xs transition-all duration-200 hover:-translate-y-1 hover:border-primary/40 hover:shadow-md'
            >
              <div>
                <div className='flex items-center gap-4 mb-4'>
                  {brand.logoUrl ? (
                    // eslint-disable-next-line @next/next/no-img-element
                    <img
                      src={brand.logoUrl}
                      alt={brand.name}
                      className='h-12 w-12 rounded-lg object-contain bg-muted p-1 border'
                    />
                  ) : (
                    <div className='flex h-12 w-12 items-center justify-center rounded-lg bg-primary/10 text-primary font-bold text-lg border'>
                      {brand.name.charAt(0).toUpperCase()}
                    </div>
                  )}
                  <div>
                    <h2 className='text-lg font-semibold text-foreground group-hover:text-primary transition-colors'>
                      {brand.name}
                    </h2>
                    <span className='text-xs font-mono text-muted-foreground'>
                      {brand.slug}
                    </span>
                  </div>
                </div>

                {brand.description && (
                  <p className='text-xs text-muted-foreground line-clamp-2 mb-4'>
                    {brand.description}
                  </p>
                )}
              </div>

              <div className='flex items-center justify-between pt-4 border-t border-border/60 text-xs text-muted-foreground'>
                <span>
                  {brand.productsCount}{' '}
                  {isRu ? 'товаров' : 'products'}
                </span>
                <span className='inline-flex items-center gap-1 font-medium text-primary group-hover:translate-x-0.5 transition-transform'>
                  {isRu ? 'Смотреть товары' : 'View products'}
                  <ArrowRight className='h-3.5 w-3.5' />
                </span>
              </div>
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}
