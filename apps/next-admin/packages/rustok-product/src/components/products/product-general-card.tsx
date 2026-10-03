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
import { Button } from '@/shared/ui/shadcn/button';
import { Badge } from '@/shared/ui/shadcn/badge';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import { Tabs, TabsList, TabsTrigger } from '@/shared/ui/shadcn/tabs';
import { Sparkles, X } from 'lucide-react';
import type { ProductTranslation } from '../../api/types';

interface ProductGeneralCardProps {
  translations: Record<string, ProductTranslation>;
  activeLocale: string;
  onActiveLocaleChange: (locale: string) => void;
  onTranslationChange: (locale: string, field: keyof ProductTranslation, value: string) => void;
  vendor: string;
  onVendorChange: (val: string) => void;
  sellerId: string;
  onSellerIdChange: (val: string) => void;
  productType: string;
  onProductTypeChange: (val: string) => void;
  shippingProfileSlug: string;
  onShippingProfileSlugChange: (val: string) => void;
  tags: string[];
  onTagsChange: (tags: string[]) => void;
  disabled?: boolean;
}

export function ProductGeneralCard({
  translations,
  activeLocale,
  onActiveLocaleChange,
  onTranslationChange,
  vendor,
  onVendorChange,
  sellerId,
  onSellerIdChange,
  productType,
  onProductTypeChange,
  shippingProfileSlug,
  onShippingProfileSlugChange,
  tags,
  onTagsChange,
  disabled = false
}: ProductGeneralCardProps) {
  const [tagInput, setTagInput] = React.useState('');
  const currentTranslation = translations[activeLocale] || {
    locale: activeLocale,
    title: '',
    handle: '',
    description: '',
    metaTitle: '',
    metaDescription: ''
  };

  const handleAutoSlug = () => {
    const titleVal = currentTranslation.title || '';
    const slug = titleVal
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9а-яё]+/gi, '-')
      .replace(/^-|-$/g, '');
    onTranslationChange(activeLocale, 'handle', slug);
  };

  const handleAddTag = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' || e.key === ',') {
      e.preventDefault();
      const val = tagInput.trim().replace(/^,|,$/g, '');
      if (val && !tags.includes(val)) {
        onTagsChange([...tags, val]);
        setTagInput('');
      }
    }
  };

  const handleRemoveTag = (tagToRemove: string) => {
    onTagsChange(tags.filter((t) => t !== tagToRemove));
  };

  return (
    <Card className='rounded-2xl border-border shadow-sm'>
      <CardHeader className='pb-4 border-b border-border/60'>
        <div className='flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3'>
          <div>
            <CardTitle className='text-sm font-semibold'>General Information</CardTitle>
            <CardDescription className='text-xs'>
              Title, URL handle, localized copy, product classification, and tagging.
            </CardDescription>
          </div>
          <Tabs
            value={activeLocale}
            onValueChange={onActiveLocaleChange}
            className='w-auto'
          >
            <TabsList className='h-8 bg-muted/60 p-0.5 rounded-lg'>
              <TabsTrigger value='en' className='text-xs px-2.5 py-1 rounded-md'>
                English (EN)
              </TabsTrigger>
              <TabsTrigger value='ru' className='text-xs px-2.5 py-1 rounded-md'>
                Русский (RU)
              </TabsTrigger>
            </TabsList>
          </Tabs>
        </div>
      </CardHeader>
      <CardContent className='pt-5 space-y-4'>
        {/* Title */}
        <div className='space-y-1.5'>
          <Label htmlFor='product-title' className='text-xs font-medium'>
            Title ({activeLocale.toUpperCase()}) <span className='text-destructive'>*</span>
          </Label>
          <Input
            id='product-title'
            value={currentTranslation.title || ''}
            onChange={(e) => onTranslationChange(activeLocale, 'title', e.target.value)}
            placeholder='e.g. Ergonomic Wireless Mouse'
            className='h-9 text-xs rounded-xl'
            disabled={disabled}
          />
        </div>

        {/* Handle */}
        <div className='space-y-1.5'>
          <div className='flex items-center justify-between'>
            <Label htmlFor='product-handle' className='text-xs font-medium'>
              URL Handle ({activeLocale.toUpperCase()})
            </Label>
            <Button
              type='button'
              variant='ghost'
              size='sm'
              onClick={handleAutoSlug}
              className='h-6 px-2 text-[11px] gap-1 text-muted-foreground hover:text-foreground'
            >
              <Sparkles className='h-3 w-3 text-amber-500' />
              <span>Generate slug</span>
            </Button>
          </div>
          <div className='flex rounded-xl border border-input focus-within:ring-1 focus-within:ring-ring overflow-hidden bg-background'>
            <span className='px-3 py-2 text-xs text-muted-foreground bg-muted/40 border-r border-border select-none'>
              /products/
            </span>
            <input
              id='product-handle'
              value={currentTranslation.handle || ''}
              onChange={(e) => onTranslationChange(activeLocale, 'handle', e.target.value)}
              placeholder='ergonomic-wireless-mouse'
              className='flex-1 px-3 py-2 text-xs bg-transparent outline-none text-foreground'
              disabled={disabled}
            />
          </div>
        </div>

        {/* Description */}
        <div className='space-y-1.5'>
          <Label htmlFor='product-desc' className='text-xs font-medium'>
            Description ({activeLocale.toUpperCase()})
          </Label>
          <Textarea
            id='product-desc'
            rows={4}
            value={currentTranslation.description || ''}
            onChange={(e) => onTranslationChange(activeLocale, 'description', e.target.value)}
            placeholder='Detailed description, features, material specifications...'
            className='text-xs rounded-xl resize-y'
            disabled={disabled}
          />
        </div>

        {/* Classification Grid */}
        <div className='grid grid-cols-1 sm:grid-cols-2 md:grid-cols-4 gap-4 pt-2'>
          <div className='space-y-1.5'>
            <Label htmlFor='product-type' className='text-xs font-medium'>
              Product Type
            </Label>
            <Select
              value={productType || 'simple'}
              onValueChange={onProductTypeChange}
              disabled={disabled}
            >
              <SelectTrigger id='product-type' className='h-9 text-xs rounded-xl'>
                <SelectValue placeholder='Select type' />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value='simple' className='text-xs'>Simple Product</SelectItem>
                <SelectItem value='variable' className='text-xs'>Variable (Matrix)</SelectItem>
                <SelectItem value='bundle' className='text-xs'>Bundle</SelectItem>
                <SelectItem value='digital' className='text-xs'>Digital Download</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div className='space-y-1.5'>
            <Label htmlFor='product-vendor' className='text-xs font-medium'>
              Vendor / Brand
            </Label>
            <Input
              id='product-vendor'
              value={vendor || ''}
              onChange={(e) => onVendorChange(e.target.value)}
              placeholder='e.g. Logitech, Apple'
              className='h-9 text-xs rounded-xl'
              disabled={disabled}
            />
          </div>

          <div className='space-y-1.5'>
            <Label htmlFor='product-seller' className='text-xs font-medium'>
              Seller ID
            </Label>
            <Input
              id='product-seller'
              value={sellerId || ''}
              onChange={(e) => onSellerIdChange(e.target.value)}
              placeholder='e.g. seller-default'
              className='h-9 text-xs rounded-xl'
              disabled={disabled}
            />
          </div>

          <div className='space-y-1.5'>
            <Label htmlFor='product-shipping' className='text-xs font-medium'>
              Shipping Profile
            </Label>
            <Input
              id='product-shipping'
              value={shippingProfileSlug || ''}
              onChange={(e) => onShippingProfileSlugChange(e.target.value)}
              placeholder='standard, fragile'
              className='h-9 text-xs rounded-xl'
              disabled={disabled}
            />
          </div>
        </div>

        {/* Tags */}
        <div className='space-y-2 pt-2'>
          <Label htmlFor='product-tags' className='text-xs font-medium'>
            Tags (press Enter or comma to add)
          </Label>
          <div className='flex flex-wrap gap-1.5 p-2 rounded-xl border border-input min-h-[42px] bg-background'>
            {tags.map((tag) => (
              <Badge
                key={tag}
                variant='secondary'
                className='text-xs gap-1 py-0.5 px-2 rounded-md bg-muted'
              >
                <span>{tag}</span>
                <button
                  type='button'
                  onClick={() => handleRemoveTag(tag)}
                  className='text-muted-foreground hover:text-foreground'
                >
                  <X className='h-3 w-3' />
                </button>
              </Badge>
            ))}
            <input
              id='product-tags'
              value={tagInput}
              onChange={(e) => setTagInput(e.target.value)}
              onKeyDown={handleAddTag}
              placeholder={tags.length === 0 ? 'gaming, wireless, ergonomic...' : ''}
              className='flex-1 text-xs bg-transparent outline-none min-w-[120px] px-1'
              disabled={disabled}
            />
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
