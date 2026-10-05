/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

'use client';

import React, { useEffect, useState } from 'react';
import type { storefrontGraphql } from '@/shared/lib/graphql';
import type {
  PricingResolutionContext,
  StorefrontPricingData
} from '../types';
import { fetchStorefrontPricing } from '../api';
import { PricingContextBar } from './pricing-context-bar';
import { PricingProductCard } from './pricing-product-card';
import { PricingProductDetailView } from './pricing-product-detail';

type GraphqlExecutor = typeof storefrontGraphql;

export interface PricingViewProps {
  initialData?: StorefrontPricingData | null;
  graphql: GraphqlExecutor;
  token?: string | null;
  tenantSlug?: string | null;
  locale?: string | null;
}

export function PricingView({
  initialData = null,
  graphql,
  token,
  tenantSlug,
  locale = 'en'
}: PricingViewProps): React.JSX.Element {
  const [data, setData] = useState<StorefrontPricingData | null>(initialData);
  const [selectedHandle, setSelectedHandle] = useState<string | null>(
    initialData?.selectedHandle ?? null
  );
  const [context, setContext] = useState<PricingResolutionContext>(
    initialData?.resolutionContext ?? {
      currencyCode: 'USD',
      quantity: 1
    }
  );
  const [loading, setLoading] = useState(!initialData);
  const [error, setError] = useState<string | null>(null);

  const isRu = locale === 'ru';
  const badge = isRu ? 'Ценообразование' : 'Pricing';
  const title = isRu
    ? 'Публичный атлас цен RusToK'
    : 'Public pricing atlas from the pricing module';
  const subtitle = isRu
    ? 'Этот раздел витрины отображает видимость цен, валютное покрытие и распродажи напрямую через модуль pricing.'
    : 'This storefront route reads pricing visibility, currency coverage and sale markers through the pricing-owned package.';

  const handleContextChange = (updated: Partial<PricingResolutionContext>) => {
    setContext((prev) => ({ ...prev, ...updated }));
  };

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    fetchStorefrontPricing(
      graphql,
      {
        selectedHandle,
        locale,
        currencyCode: context.currencyCode,
        regionId: context.regionId,
        priceListId: context.priceListId,
        channelId: context.channelId,
        channelSlug: context.channelSlug,
        quantity: context.quantity
      },
      token,
      tenantSlug
    )
      .then((res) => {
        if (!cancelled) {
          setData(res);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(
            err?.message ??
              (isRu
                ? 'Не удалось загрузить данные ценообразования'
                : 'Failed to load pricing data')
          );
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [graphql, selectedHandle, context, locale, token, tenantSlug, isRu]);

  return (
    <section className='space-y-8'>
      <header className='rounded-[2rem] border border-border bg-card p-6 shadow-sm sm:p-8'>
        <span className='inline-flex rounded-full border border-border px-3 py-1 text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground'>
          {badge}
        </span>
        <h1 className='mt-3 text-2xl font-bold text-card-foreground sm:text-3xl'>
          {title}
        </h1>
        <p className='mt-2 max-w-2xl text-sm leading-6 text-muted-foreground'>
          {subtitle}
        </p>

        <div className='mt-6'>
          <PricingContextBar
            availableChannels={data?.availableChannels ?? []}
            activePriceLists={data?.activePriceLists ?? []}
            context={context}
            onChange={handleContextChange}
            locale={locale}
          />
        </div>
      </header>

      {error ? (
        <div
          role='alert'
          className='rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive'
        >
          {error}
        </div>
      ) : null}

      {loading && !data ? (
        <div className='space-y-4'>
          <div className='h-48 animate-pulse rounded-3xl bg-muted' />
          <div className='grid gap-3 sm:grid-cols-2 lg:grid-cols-3'>
            <div className='h-32 animate-pulse rounded-2xl bg-muted' />
            <div className='h-32 animate-pulse rounded-2xl bg-muted' />
            <div className='h-32 animate-pulse rounded-2xl bg-muted' />
          </div>
        </div>
      ) : data ? (
        <div className='space-y-8'>
          {data.selectedProduct && (
            <div className='space-y-4'>
              <div className='flex items-center justify-between'>
                <h2 className='text-lg font-semibold text-card-foreground'>
                  {isRu ? 'Детализация выбранного товара' : 'Selected Product Detail'}
                </h2>
                <button
                  type='button'
                  onClick={() => setSelectedHandle(null)}
                  className='text-xs font-medium text-primary hover:underline'
                >
                  {isRu ? '✕ Сбросить выбор' : '✕ Clear selection'}
                </button>
              </div>
              <PricingProductDetailView
                product={data.selectedProduct}
                context={data.resolutionContext}
                locale={locale}
              />
            </div>
          )}

          <div className='space-y-4'>
            <div className='flex items-center justify-between'>
              <h2 className='text-lg font-semibold text-card-foreground'>
                {isRu ? 'Каталог товаров с ценами' : 'Product Pricing Catalog'}
              </h2>
              <span className='text-xs text-muted-foreground'>
                {isRu
                  ? `Всего товаров: ${data.products.total}`
                  : `Total products: ${data.products.total}`}
              </span>
            </div>

            {data.products.items.length === 0 ? (
              <div className='rounded-2xl border border-dashed border-border bg-card p-10 text-center text-sm text-muted-foreground'>
                {isRu ? 'Товары не найдены' : 'No products found'}
              </div>
            ) : (
              <div className='grid gap-4 sm:grid-cols-2 lg:grid-cols-3'>
                {data.products.items.map((prod) => (
                  <PricingProductCard
                    key={prod.id}
                    product={prod}
                    isSelected={selectedHandle === prod.handle}
                    onSelect={(handle) =>
                      setSelectedHandle((prev) => (prev === handle ? null : handle))
                    }
                    locale={locale}
                  />
                ))}
              </div>
            )}
          </div>
        </div>
      ) : null}
    </section>
  );
}
