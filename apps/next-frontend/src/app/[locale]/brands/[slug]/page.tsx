/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

import type { Metadata } from 'next';
import Link from 'next/link';
import { notFound } from 'next/navigation';
import { storefrontGraphql } from '@/shared/lib/graphql';
import { getStorefrontTenantSlug } from '@/shared/api/modules';
import { ExternalLink, Tag } from 'lucide-react';
import {
  fetchStorefrontProducts,
  ProductGrid,
  type StorefrontProductListResponse
} from '../../../../../packages/rustok-product/src';

interface BrandDetailPageProps {
  params: Promise<{ locale: string; slug: string }>;
}

interface BrandDetail {
  id: string;
  slug: string;
  name: string;
  description?: string | null;
  websiteUrl?: string | null;
  logoUrl?: string | null;
  productsCount: number;
}

const BRAND_BY_SLUG_QUERY = `
query StorefrontBrandBySlug($slug: String!, $locale: String) {
  brandBySlug(slug: $slug, locale: $locale) {
    id
    slug
    name
    description
    websiteUrl
    logoUrl
    productsCount
  }
}`;

export async function generateMetadata({
  params
}: BrandDetailPageProps): Promise<Metadata> {
  const { locale, slug } = await params;
  const tenantSlug = await getStorefrontTenantSlug();

  try {
    const res = await storefrontGraphql<{ brandBySlug: BrandDetail | null }>({
      query: BRAND_BY_SLUG_QUERY,
      variables: { slug, locale },
      tenant: tenantSlug
    });
    const brand = res.data?.brandBySlug;
    if (brand) {
      return {
        title: `${brand.name} | RusToK`,
        description: brand.description || `Products by ${brand.name}`
      };
    }
  } catch {
    // Fall back to slug title
  }

  return {
    title: `${slug} | RusToK`
  };
}

export default async function BrandDetailPage({
  params
}: BrandDetailPageProps) {
  const { locale, slug } = await params;
  const isRu = locale === 'ru';
  const tenantSlug = await getStorefrontTenantSlug();

  let brand: BrandDetail | null = null;
  try {
    const res = await storefrontGraphql<{ brandBySlug: BrandDetail | null }>({
      query: BRAND_BY_SLUG_QUERY,
      variables: { slug, locale },
      tenant: tenantSlug
    });
    brand = res.data?.brandBySlug ?? null;
  } catch {
    brand = null;
  }

  if (!brand) {
    notFound();
  }

  let productsResponse: StorefrontProductListResponse = {
    total: 0,
    page: 1,
    perPage: 24,
    hasNext: false,
    items: [],
  };

  try {
    productsResponse = await fetchStorefrontProducts(
      storefrontGraphql,
      locale,
      {
        search: brand.name,
        page: 1,
        perPage: 24
      },
      tenantSlug
    );
  } catch {
    // Graceful fallback if product catalog is unavailable
  }

  return (
    <div className='container mx-auto px-4 py-8 max-w-6xl'>
      {/* Breadcrumb */}
      <div className='flex items-center gap-2 text-xs text-muted-foreground mb-6'>
        <Link href={`/${locale}`} className='hover:underline'>
          {isRu ? 'Главная' : 'Home'}
        </Link>
        <span>/</span>
        <Link href={`/${locale}/brands`} className='hover:underline'>
          {isRu ? 'Бренды' : 'Brands'}
        </Link>
        <span>/</span>
        <span className='text-foreground'>{brand.name}</span>
      </div>

      {/* Brand Hero Header */}
      <div className='mb-10 rounded-2xl border border-border bg-card p-6 md:p-8 shadow-xs'>
        <div className='flex flex-col sm:flex-row items-start sm:items-center gap-6'>
          {brand.logoUrl ? (
            // eslint-disable-next-line @next/next/no-img-element
            <img
              src={brand.logoUrl}
              alt={brand.name}
              className='h-20 w-20 rounded-xl object-contain bg-muted p-2 border'
            />
          ) : (
            <div className='flex h-20 w-20 items-center justify-center rounded-xl bg-primary/10 text-primary font-bold text-2xl border'>
              {brand.name.charAt(0).toUpperCase()}
            </div>
          )}

          <div className='flex-1'>
            <div className='flex flex-wrap items-center gap-3 mb-2'>
              <h1 className='text-2xl md:text-3xl font-bold tracking-tight text-foreground'>
                {brand.name}
              </h1>
              <span className='inline-flex items-center rounded-full bg-primary/10 px-2.5 py-0.5 text-xs font-medium text-primary'>
                {isRu ? 'Официальный бренд' : 'Official Brand'}
              </span>
            </div>

            {brand.description && (
              <p className='text-sm text-muted-foreground max-w-3xl mb-3'>
                {brand.description}
              </p>
            )}

            {brand.websiteUrl && (
              <a
                href={brand.websiteUrl}
                target='_blank'
                rel='noreferrer'
                className='inline-flex items-center gap-1.5 text-xs font-medium text-primary hover:underline'
              >
                {isRu ? 'Официальный сайт бренда' : 'Official Website'}
                <ExternalLink className='h-3.5 w-3.5' />
              </a>
            )}
          </div>
        </div>
      </div>

      {/* Brand Products Section */}
      <div className='space-y-6'>
        <div className='flex items-center justify-between border-b pb-4'>
          <h2 className='text-xl font-semibold tracking-tight text-foreground'>
            {isRu ? 'Товары бренда' : 'Brand Products'}
          </h2>
          <span className='text-xs text-muted-foreground font-mono'>
            {productsResponse.total} {isRu ? 'товаров' : 'products'}
          </span>
        </div>

        {productsResponse.items.length === 0 ? (
          <div className='py-12 text-center text-muted-foreground'>
            <Tag className='h-10 w-10 mx-auto mb-2 opacity-40' />
            <p className='text-sm'>
              {isRu
                ? 'Для этого бренда пока нет доступных товаров.'
                : 'No products currently listed for this brand.'}
            </p>
          </div>
        ) : (
          <ProductGrid products={productsResponse.items} locale={locale} />
        )}
      </div>
    </div>
  );
}
