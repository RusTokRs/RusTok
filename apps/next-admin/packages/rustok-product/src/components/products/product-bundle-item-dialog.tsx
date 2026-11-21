/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

'use client';

import * as React from 'react';
import { toast } from 'sonner';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from '@/shared/ui/shadcn/dialog';
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import { Switch } from '@/shared/ui/shadcn/switch';
import { Search, Loader2, Package } from 'lucide-react';
import type { AddBundleItemInput } from '../../api/types';
import { isDefinitiveProductWriteRefusal } from '../../api/write-outcome';

interface SearchProductResult {
  id: string;
  title: string;
  handle?: string;
  thumbnail?: string;
  sku?: string;
  price?: string;
}

interface ProductBundleItemDialogProps {
  bundleId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAdd: (input: AddBundleItemInput, idempotencyKey: string) => Promise<void>;
  onSearchProducts?: (query: string) => Promise<SearchProductResult[]>;
  currentItemsCount: number;
  disabled?: boolean;
}

export function ProductBundleItemDialog({
  bundleId,
  open,
  onOpenChange,
  onAdd,
  onSearchProducts,
  currentItemsCount,
  disabled
}: ProductBundleItemDialogProps) {
  const [searchQuery, setSearchQuery] = React.useState('');
  const [searchResults, setSearchResults] = React.useState<
    SearchProductResult[]
  >([]);
  const [isSearching, setIsSearching] = React.useState(false);
  const [selectedProduct, setSelectedProduct] =
    React.useState<SearchProductResult | null>(null);

  const [quantityInput, setQuantityInput] = React.useState('1');
  const [isOptional, setIsOptional] = React.useState(false);
  const [discountPercent, setDiscountPercent] = React.useState('');
  const [position, setPosition] = React.useState(currentItemsCount);
  const [isSubmitting, setIsSubmitting] = React.useState(false);
  // One idempotency key per submitted intent: an ambiguous (network-level)
  // failure keeps it so a retry replays the owner receipt instead of adding
  // a duplicate bundle item; confirmed or definitively refused attempts
  // rotate it.
  const addAttemptKey = React.useRef<string | null>(null);

  const resetAddAttempt = () => {
    addAttemptKey.current = null;
  };

  React.useEffect(() => {
    if (open) {
      setSearchQuery('');
      setSearchResults([]);
      setSelectedProduct(null);
      setQuantityInput('1');
      setIsOptional(false);
      setDiscountPercent('');
      setPosition(currentItemsCount);
      resetAddAttempt();
    }
  }, [open, currentItemsCount]);

  React.useEffect(() => {
    if (!open || !onSearchProducts) return;

    if (!searchQuery.trim()) {
      setSearchResults([]);
      return;
    }

    const timer = setTimeout(async () => {
      setIsSearching(true);
      try {
        const results = await onSearchProducts(searchQuery);
        setSearchResults(results);
      } catch (err) {
        toast.error(
          err instanceof Error ? err.message : 'Failed to search products.'
        );
      } finally {
        setIsSearching(false);
      }
    }, 250);

    return () => clearTimeout(timer);
  }, [searchQuery, open, onSearchProducts]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedProduct) return;
    const quantity = Number.parseInt(quantityInput, 10);
    if (!Number.isInteger(quantity) || quantity < 1) {
      toast.error('Quantity must be a whole number of at least 1.');
      return;
    }

    let discountRate: string | undefined;
    if (discountPercent.trim()) {
      // Fail closed: an unparsable percentage must never reach the owner as
      // the string "NaN".
      const percent = Number.parseFloat(discountPercent);
      if (!Number.isFinite(percent) || percent < 0 || percent > 100) {
        toast.error('Item discount must be between 0 and 100.');
        return;
      }
      discountRate = (percent / 100).toFixed(4);
    }

    setIsSubmitting(true);
    try {
      addAttemptKey.current ??= crypto.randomUUID();
      await onAdd(
        {
          bundleId,
          productId: selectedProduct.id,
          quantity,
          isOptional,
          discountRate,
          position
        },
        addAttemptKey.current
      );
      resetAddAttempt();
      onOpenChange(false);
    } catch (err) {
      if (isDefinitiveProductWriteRefusal(err)) resetAddAttempt();
      toast.error(
        err instanceof Error ? err.message : 'Failed to add bundle item.'
      );
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className='sm:max-w-[550px]'>
        <form onSubmit={handleSubmit}>
          <DialogHeader>
            <DialogTitle>Add Product to Bundle</DialogTitle>
            <DialogDescription>
              Select an item to include in this kit or bundle composition.
            </DialogDescription>
          </DialogHeader>

          <div className='space-y-4 py-4'>
            {/* Step 1: Search & Select Product */}
            <div className='space-y-2'>
              <Label>Search Product</Label>
              <div className='relative'>
                <Search className='text-muted-foreground absolute top-2.5 left-3 h-4 w-4' />
                <Input
                  placeholder='Search by title, SKU, or handle...'
                  value={searchQuery}
                  onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                    setSearchQuery(e.target.value)
                  }
                  className='pl-9'
                  disabled={isSubmitting || disabled}
                />
                {isSearching && (
                  <Loader2 className='text-muted-foreground absolute top-2.5 right-3 h-4 w-4 animate-spin' />
                )}
              </div>

              {searchResults.length > 0 && !selectedProduct && (
                <div className='bg-popover max-h-48 overflow-y-auto rounded-md border p-1 shadow-md'>
                  {searchResults.map((product) => (
                    <div
                      key={product.id}
                      onClick={() => {
                        resetAddAttempt();
                        setSelectedProduct(product);
                      }}
                      className='hover:bg-accent hover:text-accent-foreground flex cursor-pointer items-center justify-between rounded px-3 py-2 text-sm'
                    >
                      <div className='flex items-center gap-2'>
                        <div className='bg-muted flex h-8 w-8 items-center justify-center rounded'>
                          {product.thumbnail ? (
                            <img
                              src={product.thumbnail}
                              alt={product.title}
                              className='h-full w-full rounded object-cover'
                            />
                          ) : (
                            <Package className='text-muted-foreground h-4 w-4' />
                          )}
                        </div>
                        <div>
                          <p className='font-medium'>{product.title}</p>
                          {product.sku && (
                            <p className='text-muted-foreground text-xs'>
                              SKU: {product.sku}
                            </p>
                          )}
                        </div>
                      </div>
                      {product.price && (
                        <span className='font-mono text-xs'>
                          {product.price}
                        </span>
                      )}
                    </div>
                  ))}
                </div>
              )}

              {selectedProduct && (
                <div className='border-primary/40 bg-primary/5 flex items-center justify-between rounded-lg border p-3 text-sm'>
                  <div className='flex items-center gap-3'>
                    <div className='bg-background flex h-10 w-10 items-center justify-center rounded shadow-xs'>
                      {selectedProduct.thumbnail ? (
                        <img
                          src={selectedProduct.thumbnail}
                          alt={selectedProduct.title}
                          className='h-full w-full rounded object-cover'
                        />
                      ) : (
                        <Package className='text-muted-foreground h-5 w-5' />
                      )}
                    </div>
                    <div>
                      <p className='font-semibold'>{selectedProduct.title}</p>
                      <p className='text-muted-foreground text-xs'>
                        {selectedProduct.sku
                          ? `SKU: ${selectedProduct.sku}`
                          : selectedProduct.id}
                      </p>
                    </div>
                  </div>
                  <Button
                    type='button'
                    variant='ghost'
                    size='sm'
                    onClick={() => {
                      resetAddAttempt();
                      setSelectedProduct(null);
                    }}
                    disabled={isSubmitting}
                  >
                    Change
                  </Button>
                </div>
              )}
            </div>

            {/* Step 2: Configuration */}
            {selectedProduct && (
              <div className='bg-muted/20 space-y-4 rounded-md border p-3'>
                <div className='grid grid-cols-2 gap-4'>
                  <div className='space-y-1'>
                    <Label htmlFor='bundle-item-qty'>Quantity</Label>
                    <Input
                      id='bundle-item-qty'
                      type='number'
                      min={1}
                      value={quantityInput}
                      onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                        resetAddAttempt();
                        setQuantityInput(e.target.value);
                      }}
                      disabled={isSubmitting || disabled}
                    />
                  </div>

                  <div className='space-y-1'>
                    <Label htmlFor='bundle-item-discount'>
                      Item Discount (%)
                    </Label>
                    <Input
                      id='bundle-item-discount'
                      type='number'
                      min={0}
                      max={100}
                      step={0.5}
                      placeholder='e.g. 10'
                      value={discountPercent}
                      onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                        resetAddAttempt();
                        setDiscountPercent(e.target.value);
                      }}
                      disabled={isSubmitting || disabled}
                    />
                  </div>
                </div>

                <div className='flex items-center justify-between pt-2'>
                  <div className='space-y-0.5'>
                    <Label className='text-sm'>Optional Item</Label>
                    <p className='text-muted-foreground text-xs'>
                      Allow customers to deselect or swap this item in flexible
                      kits.
                    </p>
                  </div>
                  <Switch
                    checked={isOptional}
                    onCheckedChange={(checked) => {
                      resetAddAttempt();
                      setIsOptional(checked);
                    }}
                    disabled={isSubmitting || disabled}
                  />
                </div>
              </div>
            )}
          </div>

          <DialogFooter>
            <Button
              type='button'
              variant='outline'
              onClick={() => onOpenChange(false)}
              disabled={isSubmitting}
            >
              Cancel
            </Button>
            <Button
              type='submit'
              disabled={!selectedProduct || isSubmitting || disabled}
            >
              {isSubmitting ? (
                <>
                  <Loader2 className='mr-2 h-4 w-4 animate-spin' />
                  Adding...
                </>
              ) : (
                'Add to Bundle'
              )}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
