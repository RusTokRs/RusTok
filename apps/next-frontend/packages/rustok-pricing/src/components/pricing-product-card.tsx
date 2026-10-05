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
import type { PricingProductListItem } from '../types';

export function PricingProductCard({
  product,
  isSelected,
  onSelect,
  locale = 'en'
}: {
  product: PricingProductListItem;
  isSelected?: boolean;
  onSelect: (handle: string) => void;
  locale?: string | null;
}): React.JSX.Element {
  const isRu = locale === 'ru';

  return (
    <article
      onClick={() => onSelect(product.handle)}
      className={`group cursor-pointer rounded-2xl border p-5 transition-all hover:shadow-md ${
        isSelected
          ? 'border-primary bg-primary/5 shadow-sm'
          : 'border-border bg-card hover:border-primary/50'
      }`}
    >
      <div className='flex items-start justify-between gap-2'>
        <span className='rounded-full border border-border bg-background px-2.5 py-0.5 font-mono text-[10px] text-muted-foreground uppercase'>
          {product.productType || 'Product'}
        </span>
        {product.saleVariantCount > 0 && (
          <span className='rounded-full bg-rose-100 px-2 py-0.5 text-[10px] font-semibold text-rose-700 dark:bg-rose-900/30 dark:text-rose-400'>
            {isRu ? 'Скидка' : 'Sale'}
          </span>
        )}
      </div>

      <h3 className='mt-3 text-base font-semibold text-card-foreground group-hover:text-primary transition-colors'>
        {product.title}
      </h3>
      <p className='mt-1 text-xs text-muted-foreground'>
        {product.vendor ? `${product.vendor} · ` : ''}
        <span className='font-mono'>@{product.handle}</span>
      </p>

      <div className='mt-4 flex items-center justify-between border-t border-border/50 pt-3 text-xs text-muted-foreground'>
        <span>
          {isRu
            ? `${product.variantCount} вар.`
            : `${product.variantCount} var.`}
        </span>
        <span className='font-medium text-primary'>
          {isSelected
            ? isRu
              ? 'Выбран'
              : 'Selected'
            : isRu
              ? 'Смотреть цены →'
              : 'Inspect prices →'}
        </span>
      </div>
    </article>
  );
}
