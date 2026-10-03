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
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription
} from '@/shared/ui/shadcn/card';
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
  const displayTitle =
    translation.metaTitle || translation.title || 'Product Title';
  const displayDesc =
    translation.metaDescription ||
    translation.description ||
    'Provide an explicit meta description to improve storefront visibility and click-through rates in search engine results.';
  const displaySlug = translation.handle || 'product-handle';

  return (
    <Card className='border-border rounded-2xl shadow-sm'>
      <CardHeader className='border-border/60 border-b pb-4'>
        <div className='flex items-center gap-2'>
          <Search className='text-primary h-4 w-4' />
          <div>
            <CardTitle className='text-sm font-semibold'>
              Search Engine Optimization
            </CardTitle>
            <CardDescription className='text-xs'>
              Explicit meta tags and SERP search snippet preview (
              {activeLocale.toUpperCase()}).
            </CardDescription>
          </div>
        </div>
      </CardHeader>
      <CardContent className='space-y-5 pt-5'>
        {/* SERP Preview Box */}
        <div className='border-border/80 bg-muted/20 space-y-1 rounded-xl border p-4'>
          <div className='text-muted-foreground flex items-center gap-1.5 text-[11px]'>
            <Globe className='text-muted-foreground/70 h-3 w-3' />
            <span className='truncate'>
              https://example.com › products › {displaySlug}
            </span>
          </div>
          <div className='cursor-pointer truncate text-sm font-medium text-blue-600 hover:underline dark:text-blue-400'>
            {displayTitle}
          </div>
          <p className='text-muted-foreground line-clamp-2 text-xs leading-relaxed'>
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
              <span className='text-muted-foreground text-[10px]'>
                {(translation.metaTitle || '').length} / 60 chars
              </span>
            </div>
            <Input
              id='meta-title'
              value={translation.metaTitle || ''}
              onChange={(e) => onChange('metaTitle', e.target.value)}
              placeholder={translation.title || 'Defaults to product title'}
              className='h-9 rounded-xl text-xs'
              disabled={disabled}
            />
          </div>

          <div className='space-y-1.5'>
            <div className='flex items-center justify-between'>
              <Label htmlFor='meta-desc' className='text-xs font-medium'>
                Meta Description
              </Label>
              <span className='text-muted-foreground text-[10px]'>
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
              className='resize-none rounded-xl text-xs'
              disabled={disabled}
            />
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
