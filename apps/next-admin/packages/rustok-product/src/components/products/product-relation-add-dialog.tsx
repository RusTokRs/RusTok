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
import { isDefinitiveProductWriteRefusal } from '../../api/write-outcome';
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
  onAddRelation: (
    input: AddProductRelationInput,
    idempotencyKey: string
  ) => Promise<void>;
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
    label: 'Cross-sell',
    description: 'Recommended in the cart or at checkout',
    color:
      'bg-blue-50 text-blue-700 border-blue-200 dark:bg-blue-950/40 dark:text-blue-300 dark:border-blue-800'
  },
  {
    value: 'UP_SELL',
    label: 'Up-sell',
    description: 'A more advanced or premium alternative',
    color:
      'bg-purple-50 text-purple-700 border-purple-200 dark:bg-purple-950/40 dark:text-purple-300 dark:border-purple-800'
  },
  {
    value: 'RELATED',
    label: 'Related',
    description: 'Products from the same category or similar use',
    color:
      'bg-emerald-50 text-emerald-700 border-emerald-200 dark:bg-emerald-950/40 dark:text-emerald-300 dark:border-emerald-800'
  },
  {
    value: 'ACCESSORY',
    label: 'Accessory',
    description: 'Cases, cables, consumables, or compatible add-ons',
    color:
      'bg-amber-50 text-amber-700 border-amber-200 dark:bg-amber-950/40 dark:text-amber-300 dark:border-amber-800'
  },
  {
    value: 'ALTERNATIVE',
    label: 'Alternative',
    description: 'A direct substitute when the main product is out of stock',
    color:
      'bg-slate-50 text-slate-700 border-slate-200 dark:bg-slate-950/40 dark:text-slate-300 dark:border-slate-800'
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
  const [searchResults, setSearchResults] = React.useState<ProductListItem[]>(
    []
  );
  const [selectedProduct, setSelectedProduct] =
    React.useState<ProductListItem | null>(null);
  const [relationType, setRelationType] =
    React.useState<ProductRelationType>('CROSS_SELL');
  const [positionInput, setPositionInput] = React.useState('0');
  const [isSubmitting, setIsSubmitting] = React.useState(false);
  const [errorMessage, setErrorMessage] = React.useState<string | null>(null);
  // One idempotency key per submitted intent: an ambiguous (network-level)
  // failure keeps it so a retry replays the owner receipt instead of creating
  // a duplicate relation; confirmed or definitively refused attempts rotate it.
  const addAttemptKey = React.useRef<string | null>(null);

  const resetAddAttempt = () => {
    addAttemptKey.current = null;
  };

  // Debounced search
  React.useEffect(() => {
    if (!open) {
      setSearchQuery('');
      setSearchResults([]);
      setSelectedProduct(null);
      setRelationType('CROSS_SELL');
      setPositionInput('0');
      setErrorMessage(null);
      resetAddAttempt();
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
    const position = Number.parseInt(positionInput, 10);
    if (!Number.isInteger(position) || position < 0) {
      setErrorMessage('Position must be a non-negative whole number.');
      return;
    }

    setIsSubmitting(true);
    setErrorMessage(null);
    addAttemptKey.current ??= crypto.randomUUID();
    try {
      await onAddRelation(
        {
          productId,
          relatedProductId: selectedProduct.id,
          relationType,
          position
        },
        addAttemptKey.current
      );
      resetAddAttempt();
      onOpenChange(false);
    } catch (err: unknown) {
      if (isDefinitiveProductWriteRefusal(err)) resetAddAttempt();
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
              Add Product Relation
            </DialogTitle>
          </DialogHeader>

          {errorMessage && (
            <div className='border-destructive/30 bg-destructive/10 text-destructive rounded-xl border p-3 text-xs'>
              {errorMessage}
            </div>
          )}

          {/* Product Search & Picker */}
          <div className='space-y-2'>
            <Label className='text-xs font-medium'>
              Target Product <span className='text-destructive'>*</span>
            </Label>
            <div className='relative'>
              <Search className='text-muted-foreground absolute top-2.5 left-3 h-4 w-4' />
              <Input
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder='Search by title or handle...'
                className='h-9 rounded-xl pl-9 text-xs'
                disabled={disabled || isSubmitting}
              />
              {isSearching && (
                <Loader2 className='text-muted-foreground absolute top-2.5 right-3 h-4 w-4 animate-spin' />
              )}
            </div>

            {/* Selected Product Card */}
            {selectedProduct && (
              <div className='border-primary/30 bg-primary/5 flex items-center justify-between rounded-xl border p-2.5'>
                <div className='flex items-center gap-2.5'>
                  <div className='bg-primary/10 text-primary flex h-8 w-8 items-center justify-center rounded-lg'>
                    <Package className='h-4 w-4' />
                  </div>
                  <div>
                    <div className='text-foreground text-xs font-semibold'>
                      {selectedProduct.title}
                    </div>
                    <div className='text-muted-foreground text-[10px]'>
                      handle: {selectedProduct.handle} • id:{' '}
                      {selectedProduct.id.slice(0, 8)}...
                    </div>
                  </div>
                </div>
                <Badge variant='outline' className='text-[10px]'>
                  Selected
                </Badge>
              </div>
            )}

            {/* Search Results List */}
            {searchResults.length > 0 && !selectedProduct && (
              <div className='border-border bg-background max-h-48 space-y-1 overflow-y-auto rounded-xl border p-1 shadow-sm'>
                {searchResults.map((item) => (
                  <button
                    key={item.id}
                    type='button'
                    onClick={() => {
                      resetAddAttempt();
                      setSelectedProduct(item);
                      setSearchResults([]);
                    }}
                    className='hover:bg-accent flex w-full items-center justify-between rounded-lg p-2 text-left text-xs transition'
                  >
                    <div>
                      <div className='text-foreground font-medium'>
                        {item.title}
                      </div>
                      <div className='text-muted-foreground text-[10px]'>
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
              Relation Type <span className='text-destructive'>*</span>
            </Label>
            <div className='grid grid-cols-1 gap-2'>
              {RELATION_TYPE_OPTIONS.map((opt) => {
                const isSelected = relationType === opt.value;
                return (
                  <button
                    key={opt.value}
                    type='button'
                    onClick={() => {
                      resetAddAttempt();
                      setRelationType(opt.value);
                    }}
                    className={`flex items-start justify-between rounded-xl border p-2.5 text-left transition ${
                      isSelected
                        ? 'border-primary bg-primary/5 ring-primary ring-1'
                        : 'border-border hover:bg-accent/40'
                    }`}
                  >
                    <div>
                      <div className='flex items-center gap-2'>
                        <span className='text-foreground text-xs font-semibold'>
                          {opt.label}
                        </span>
                        <Badge
                          variant='outline'
                          className={`px-1.5 py-0 text-[9px] uppercase ${opt.color}`}
                        >
                          {opt.value}
                        </Badge>
                      </div>
                      <p className='text-muted-foreground mt-0.5 text-[11px]'>
                        {opt.description}
                      </p>
                    </div>
                    {isSelected && (
                      <Check className='text-primary mt-0.5 h-4 w-4 shrink-0' />
                    )}
                  </button>
                );
              })}
            </div>
          </div>

          {/* Position Input */}
          <div className='space-y-1.5'>
            <Label className='text-xs font-medium'>
              Position
            </Label>
            <Input
              type='number'
              min='0'
              value={positionInput}
              onChange={(e) => {
                resetAddAttempt();
                setPositionInput(e.target.value);
              }}
              className='h-9 rounded-xl font-mono text-xs'
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
              Cancel
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
                  Saving...
                </>
              ) : (
                'Add Relation'
              )}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
