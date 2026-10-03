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

import * as React from 'react';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/shared/ui/shadcn/card';
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import { Textarea } from '@/shared/ui/shadcn/textarea';
import { Search, Globe } from 'lucide-react';
import type { ProductTranslation } from '../../api/types';

interface ProductSeoCardProps {
  translation: ProductTranslation;
  activeLocale: string;
  onChange: (field: keyof ProductTranslation, value: string) => void;
  disabled?: boolean;
}

export function ProductSeoCard({
  translation,
  activeLocale,
  onChange,
  disabled = false
}: ProductSeoCardProps) {
  const displayTitle = translation.metaTitle || translation.title || 'Product Title';
  const displayDesc =
    translation.metaDescription ||
    translation.description ||
    'Provide an explicit meta description to improve storefront visibility and click-through rates in search engine results.';
  const displaySlug = translation.handle || 'product-handle';

  return (
    <Card className='rounded-2xl border-border shadow-sm'>
      <CardHeader className='pb-4 border-b border-border/60'>
        <div className='flex items-center gap-2'>
          <Search className='h-4 w-4 text-primary' />
          <div>
            <CardTitle className='text-sm font-semibold'>Search Engine Optimization</CardTitle>
            <CardDescription className='text-xs'>
              Explicit meta tags and SERP search snippet preview ({activeLocale.toUpperCase()}).
            </CardDescription>
          </div>
        </div>
      </CardHeader>
      <CardContent className='pt-5 space-y-5'>
        {/* SERP Preview Box */}
        <div className='p-4 rounded-xl border border-border/80 bg-muted/20 space-y-1'>
          <div className='flex items-center gap-1.5 text-[11px] text-muted-foreground'>
            <Globe className='h-3 w-3 text-muted-foreground/70' />
            <span className='truncate'>https://example.com › products › {displaySlug}</span>
          </div>
          <div className='text-sm font-medium text-blue-600 dark:text-blue-400 hover:underline cursor-pointer truncate'>
            {displayTitle}
          </div>
          <p className='text-xs text-muted-foreground line-clamp-2 leading-relaxed'>
            {displayDesc}
          </p>
        </div>

        {/* Inputs */}
        <div className='space-y-4'>
          <div className='space-y-1.5'>
            <div className='flex items-center justify-between'>
              <Label htmlFor='meta-title' className='text-xs font-medium'>
                Meta Title
              </Label>
              <span className='text-[10px] text-muted-foreground'>
                {(translation.metaTitle || '').length} / 60 chars
              </span>
            </div>
            <Input
              id='meta-title'
              value={translation.metaTitle || ''}
              onChange={(e) => onChange('metaTitle', e.target.value)}
              placeholder={translation.title || 'Defaults to product title'}
              className='h-9 text-xs rounded-xl'
              disabled={disabled}
            />
          </div>

          <div className='space-y-1.5'>
            <div className='flex items-center justify-between'>
              <Label htmlFor='meta-desc' className='text-xs font-medium'>
                Meta Description
              </Label>
              <span className='text-[10px] text-muted-foreground'>
                {(translation.metaDescription || '').length} / 160 chars
              </span>
            </div>
            <Textarea
              id='meta-desc'
              rows={3}
              value={translation.metaDescription || ''}
              onChange={(e) => onChange('metaDescription', e.target.value)}
              placeholder={
                translation.description
                  ? translation.description.slice(0, 160)
                  : 'Defaults to product description snippet'
              }
              className='text-xs rounded-xl resize-none'
              disabled={disabled}
            />
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
