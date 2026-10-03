/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

'use strict';

import * as React from 'react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter
} from '@/shared/ui/shadcn/dialog';
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Search, Loader2, Package, Check } from 'lucide-react';
import type {
  ProductListItem,
  ProductRelationType,
  AddProductRelationInput
} from '../../api/types';

interface ProductRelationAddDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  productId: string;
  onSearchProducts?: (query: string) => Promise<ProductListItem[]>;
  onAddRelation: (input: AddProductRelationInput) => Promise<void>;
  disabled?: boolean;
}

const RELATION_TYPE_OPTIONS: Array<{
  value: ProductRelationType;
  label: string;
  description: string;
  color: string;
}> = [
  {
    value: 'CROSS_SELL',
    label: 'Cross-sell (Сопутствующий)',
    description: 'Рекомендуется в корзине или при оформлении заказа',
    color: 'bg-blue-50 text-blue-700 border-blue-200 dark:bg-blue-950/40 dark:text-blue-300 dark:border-blue-800'
  },
  {
    value: 'UP_SELL',
    label: 'Up-sell (Апселл)',
    description: 'Более продвинутая или премиальная альтернатива',
    color: 'bg-purple-50 text-purple-700 border-purple-200 dark:bg-purple-950/40 dark:text-purple-300 dark:border-purple-800'
  },
  {
    value: 'RELATED',
    label: 'Related (Похожий)',
    description: 'Товары из той же категории или схожего назначения',
    color: 'bg-emerald-50 text-emerald-700 border-emerald-200 dark:bg-emerald-950/40 dark:text-emerald-300 dark:border-emerald-800'
  },
  {
    value: 'ACCESSORY',
    label: 'Accessory (Аксессуар)',
    description: 'Чехлы, кабели, расходники или совместимые дополнения',
    color: 'bg-amber-50 text-amber-700 border-amber-200 dark:bg-amber-950/40 dark:text-amber-300 dark:border-amber-800'
  },
  {
    value: 'ALTERNATIVE',
    label: 'Alternative (Альтернатива)',
    description: 'Прямая замена, если основной товар временно закончился',
    color: 'bg-slate-50 text-slate-700 border-slate-200 dark:bg-slate-950/40 dark:text-slate-300 dark:border-slate-800'
  }
];

export function ProductRelationAddDialog({
  open,
  onOpenChange,
  productId,
  onSearchProducts,
  onAddRelation,
  disabled
}: ProductRelationAddDialogProps) {
  const [searchQuery, setSearchQuery] = React.useState('');
  const [isSearching, setIsSearching] = React.useState(false);
  const [searchResults, setSearchResults] = React.useState<ProductListItem[]>([]);
  const [selectedProduct, setSelectedProduct] = React.useState<ProductListItem | null>(null);
  const [relationType, setRelationType] = React.useState<ProductRelationType>('CROSS_SELL');
  const [position, setPosition] = React.useState<number>(0);
  const [isSubmitting, setIsSubmitting] = React.useState(false);
  const [errorMessage, setErrorMessage] = React.useState<string | null>(null);

  // Debounced search
  React.useEffect(() => {
    if (!open) {
      setSearchQuery('');
      setSearchResults([]);
      setSelectedProduct(null);
      setRelationType('CROSS_SELL');
      setPosition(0);
      setErrorMessage(null);
      return;
    }

    if (!onSearchProducts) return;

    const timer = setTimeout(async () => {
      setIsSearching(true);
      setErrorMessage(null);
      try {
        const results = await onSearchProducts(searchQuery.trim());
        // Filter out current product
        setSearchResults(results.filter((p) => p.id !== productId));
      } catch (err: unknown) {
        setErrorMessage(
          err instanceof Error ? err.message : 'Failed to search products'
        );
      } finally {
        setIsSearching(false);
      }
    }, 300);

    return () => clearTimeout(timer);
  }, [searchQuery, open, productId, onSearchProducts]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedProduct) {
      setErrorMessage('Please select a target product to relate.');
      return;
    }

    setIsSubmitting(true);
    setErrorMessage(null);
    try {
      await onAddRelation({
        productId,
        relatedProductId: selectedProduct.id,
        relationType,
        position
      });
      onOpenChange(false);
    } catch (err: unknown) {
      setErrorMessage(
        err instanceof Error ? err.message : 'Failed to add relation'
      );
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className='max-w-lg rounded-2xl'>
        <form onSubmit={handleSubmit} className='space-y-4'>
          <DialogHeader>
            <DialogTitle className='text-base font-semibold'>
              Добавить связь с товаром
            </DialogTitle>
          </DialogHeader>

          {errorMessage && (
            <div className='rounded-xl border border-destructive/30 bg-destructive/10 p-3 text-xs text-destructive'>
              {errorMessage}
            </div>
          )}

          {/* Product Search & Picker */}
          <div className='space-y-2'>
            <Label className='text-xs font-medium'>
              Целевой товар <span className='text-destructive'>*</span>
            </Label>
            <div className='relative'>
              <Search className='absolute left-3 top-2.5 h-4 w-4 text-muted-foreground' />
              <Input
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder='Поиск по названию или handle...'
                className='h-9 rounded-xl pl-9 text-xs'
                disabled={disabled || isSubmitting}
              />
              {isSearching && (
                <Loader2 className='absolute right-3 top-2.5 h-4 w-4 animate-spin text-muted-foreground' />
              )}
            </div>

            {/* Selected Product Card */}
            {selectedProduct && (
              <div className='flex items-center justify-between rounded-xl border border-primary/30 bg-primary/5 p-2.5'>
                <div className='flex items-center gap-2.5'>
                  <div className='flex h-8 w-8 items-center justify-center rounded-lg bg-primary/10 text-primary'>
                    <Package className='h-4 w-4' />
                  </div>
                  <div>
                    <div className='text-xs font-semibold text-foreground'>
                      {selectedProduct.title}
                    </div>
                    <div className='text-[10px] text-muted-foreground'>
                      handle: {selectedProduct.handle} • id: {selectedProduct.id.slice(0, 8)}...
                    </div>
                  </div>
                </div>
                <Badge variant='outline' className='text-[10px]'>
                  Выбран
                </Badge>
              </div>
            )}

            {/* Search Results List */}
            {searchResults.length > 0 && !selectedProduct && (
              <div className='max-h-48 overflow-y-auto rounded-xl border border-border bg-background p-1 space-y-1 shadow-sm'>
                {searchResults.map((item) => (
                  <button
                    key={item.id}
                    type='button'
                    onClick={() => {
                      setSelectedProduct(item);
                      setSearchResults([]);
                    }}
                    className='w-full flex items-center justify-between rounded-lg p-2 text-left text-xs hover:bg-accent transition'
                  >
                    <div>
                      <div className='font-medium text-foreground'>{item.title}</div>
                      <div className='text-[10px] text-muted-foreground'>
                        {item.handle}
                      </div>
                    </div>
                    <Badge variant='outline' className='text-[10px] uppercase'>
                      {item.status}
                    </Badge>
                  </button>
                ))}
              </div>
            )}
          </div>

          {/* Relation Type Picker */}
          <div className='space-y-2'>
            <Label className='text-xs font-medium'>
              Тип связи <span className='text-destructive'>*</span>
            </Label>
            <div className='grid grid-cols-1 gap-2'>
              {RELATION_TYPE_OPTIONS.map((opt) => {
                const isSelected = relationType === opt.value;
                return (
                  <button
                    key={opt.value}
                    type='button'
                    onClick={() => setRelationType(opt.value)}
                    className={`flex items-start justify-between rounded-xl border p-2.5 text-left transition ${
                      isSelected
                        ? 'border-primary bg-primary/5 ring-1 ring-primary'
                        : 'border-border hover:bg-accent/40'
                    }`}
                  >
                    <div>
                      <div className='flex items-center gap-2'>
                        <span className='text-xs font-semibold text-foreground'>
                          {opt.label}
                        </span>
                        <Badge
                          variant='outline'
                          className={`text-[9px] uppercase px-1.5 py-0 ${opt.color}`}
                        >
                          {opt.value}
                        </Badge>
                      </div>
                      <p className='text-[11px] text-muted-foreground mt-0.5'>
                        {opt.description}
                      </p>
                    </div>
                    {isSelected && (
                      <Check className='h-4 w-4 text-primary shrink-0 mt-0.5' />
                    )}
                  </button>
                );
              })}
            </div>
          </div>

          {/* Position Input */}
          <div className='space-y-1.5'>
            <Label className='text-xs font-medium'>
              Порядковый номер (Position)
            </Label>
            <Input
              type='number'
              min='0'
              value={position}
              onChange={(e) => setPosition(parseInt(e.target.value, 10) || 0)}
              className='h-9 rounded-xl text-xs font-mono'
              disabled={disabled || isSubmitting}
            />
          </div>

          <DialogFooter className='pt-2'>
            <Button
              type='button'
              variant='outline'
              size='sm'
              onClick={() => onOpenChange(false)}
              disabled={isSubmitting}
              className='rounded-xl'
            >
              Отмена
            </Button>
            <Button
              type='submit'
              size='sm'
              disabled={disabled || isSubmitting || !selectedProduct}
              className='rounded-xl'
            >
              {isSubmitting ? (
                <>
                  <Loader2 className='mr-1.5 h-3.5 w-3.5 animate-spin' />
                  Сохранение...
                </>
              ) : (
                'Добавить связь'
              )}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
