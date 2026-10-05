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

import React from 'react';
import type { PricingProductDetail, PricingResolutionContext } from '../types';

export function PricingProductDetailView({
  product,
  context,
  locale = 'en'
}: {
  product: PricingProductDetail;
  context?: PricingResolutionContext | null;
  locale?: string | null;
}): React.JSX.Element {
  const isRu = locale === 'ru';
  const translation =
    product.translations.find((t) => t.locale === locale) ||
    product.translations[0];
  const title = translation?.title || product.id;
  const description = translation?.description;

  return (
    <div className='space-y-6 rounded-[2rem] border border-border bg-card p-6 shadow-sm sm:p-8'>
      <div>
        <div className='flex items-center gap-2'>
          <span className='rounded-full border border-border px-3 py-1 font-mono text-xs text-muted-foreground uppercase'>
            {product.productType || 'Product'}
          </span>
          <span className='rounded-full bg-emerald-100 px-3 py-1 text-xs font-semibold text-emerald-800 dark:bg-emerald-900/30 dark:text-emerald-400 capitalize'>
            {product.status}
          </span>
        </div>
        <h2 className='mt-3 text-2xl font-bold text-card-foreground sm:text-3xl'>
          {title}
        </h2>
        {description && (
          <p className='mt-2 max-w-3xl text-sm leading-6 text-muted-foreground'>
            {description}
          </p>
        )}
      </div>

      <div className='space-y-4'>
        <h3 className='text-sm font-semibold text-card-foreground uppercase tracking-[0.14em]'>
          {isRu ? 'Матрица цен вариантов' : 'Variant Price Matrix'}
        </h3>

        <div className='overflow-x-auto rounded-2xl border border-border'>
          <table className='w-full text-left text-sm'>
            <thead className='border-b border-border bg-muted/40 text-xs font-semibold text-muted-foreground uppercase tracking-wider'>
              <tr>
                <th className='px-4 py-3'>{isRu ? 'Вариант' : 'Variant'}</th>
                <th className='px-4 py-3'>SKU</th>
                <th className='px-4 py-3'>{isRu ? 'Базовая цена' : 'Base Price'}</th>
                <th className='px-4 py-3'>
                  {isRu ? 'Эффективная цена' : 'Effective Price'}
                </th>
                <th className='px-4 py-3'>{isRu ? 'Статус' : 'Status'}</th>
              </tr>
            </thead>
            <tbody className='divide-y divide-border'>
              {product.variants.map((variant) => {
                const effective = variant.effectivePrice;
                const basePrice =
                  variant.prices.find(
                    (p) => p.currencyCode === (context?.currencyCode ?? 'USD')
                  ) || variant.prices[0];

                return (
                  <tr key={variant.id} className='hover:bg-muted/20'>
                    <td className='px-4 py-3 font-medium text-card-foreground'>
                      {variant.title}
                    </td>
                    <td className='px-4 py-3 font-mono text-xs text-muted-foreground'>
                      {variant.sku}
                    </td>
                    <td className='px-4 py-3 text-muted-foreground'>
                      {basePrice ? `${basePrice.amount} ${basePrice.currencyCode}` : '—'}
                    </td>
                    <td className='px-4 py-3'>
                      {effective ? (
                        <div className='flex items-baseline gap-2'>
                          <span className='font-semibold text-primary text-base'>
                            {effective.amount} {effective.currencyCode}
                          </span>
                          {effective.compareAtAmount && (
                            <span className='text-xs text-muted-foreground line-through'>
                              {effective.compareAtAmount}
                            </span>
                          )}
                          {effective.discountPercent != null && effective.discountPercent > 0 && (
                            <span className='rounded-full bg-rose-100 px-2 py-0.5 text-[10px] font-semibold text-rose-700 dark:bg-rose-900/30 dark:text-rose-400'>
                              -{effective.discountPercent}%
                            </span>
                          )}
                        </div>
                      ) : basePrice ? (
                        <span className='font-semibold text-foreground'>
                          {basePrice.amount} {basePrice.currencyCode}
                        </span>
                      ) : (
                        '—'
                      )}
                    </td>
                    <td className='px-4 py-3'>
                      {effective?.onSale || basePrice?.onSale ? (
                        <span className='inline-flex rounded-full bg-emerald-100 px-2.5 py-0.5 text-xs font-semibold text-emerald-800 dark:bg-emerald-900/30 dark:text-emerald-400'>
                          {isRu ? 'Распродажа' : 'On Sale'}
                        </span>
                      ) : (
                        <span className='inline-flex rounded-full bg-muted px-2.5 py-0.5 text-xs text-muted-foreground'>
                          {isRu ? 'Стандарт' : 'Standard'}
                        </span>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
