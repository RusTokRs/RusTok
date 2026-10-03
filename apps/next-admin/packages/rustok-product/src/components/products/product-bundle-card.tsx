/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

'use client';

import * as React from 'react';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import { Badge } from '@/shared/ui/shadcn/badge';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from '@/widgets/data-table';
import {
  Boxes,
  Plus,
  Trash2,
  Package,
  Sparkles,
  Percent,
  CheckCircle2,
  Layers
} from 'lucide-react';
import type {
  ProductBundle,
  BundleItem,
  CreateBundleInput,
  UpdateBundleInput,
  AddBundleItemInput
} from '../../api/types';
import { ProductBundleItemDialog } from './product-bundle-item-dialog';

interface ProductBundleCardProps {
  productId: string;
  productTitle: string;
  productHandle?: string;
  bundle?: ProductBundle | null;
  onCreateBundle?: (input: CreateBundleInput) => Promise<void>;
  onUpdateBundle?: (bundleId: string, input: UpdateBundleInput) => Promise<void>;
  onAddBundleItem?: (input: AddBundleItemInput) => Promise<void>;
  onRemoveBundleItem?: (bundleId: string, itemId: string) => Promise<void>;
  onSearchProducts?: (query: string) => Promise<{
    id: string;
    title: string;
    handle?: string;
    thumbnail?: string;
    sku?: string;
    price?: string;
  }[]>;
  disabled?: boolean;
}

export function ProductBundleCard({
  productId,
  productTitle,
  productHandle,
  bundle,
  onCreateBundle,
  onUpdateBundle,
  onAddBundleItem,
  onRemoveBundleItem,
  onSearchProducts,
  disabled
}: ProductBundleCardProps) {
  const [isItemDialogOpen, setIsItemDialogOpen] = React.useState(false);
  const [isUpdating, setIsUpdating] = React.useState(false);

  // Editable bundle config state
  const [bundleType, setBundleType] = React.useState(bundle?.bundleType || 'fixed');
  const [status, setStatus] = React.useState(bundle?.status || 'active');
  const [discountType, setDiscountType] = React.useState(bundle?.discountType || 'none');
  const [discountValue, setDiscountValue] = React.useState(bundle?.discountValue || '0');

  React.useEffect(() => {
    if (bundle) {
      setBundleType(bundle.bundleType);
      setStatus(bundle.status);
      setDiscountType(bundle.discountType);
      setDiscountValue(bundle.discountValue);
    }
  }, [bundle]);

  const hasConfigChanges =
    bundle &&
    (bundleType !== bundle.bundleType ||
      status !== bundle.status ||
      discountType !== bundle.discountType ||
      discountValue !== bundle.discountValue);

  const handleSaveConfig = async () => {
    if (!bundle || !onUpdateBundle) return;
    setIsUpdating(true);
    try {
      await onUpdateBundle(bundle.id, {
        bundleType,
        status,
        discountType,
        discountValue
      });
    } catch (err) {
      console.error('Failed to update bundle settings:', err);
    } finally {
      setIsUpdating(false);
    }
  };

  const handleInitBundle = async () => {
    if (!onCreateBundle) return;
    setIsUpdating(true);
    try {
      const slug = productHandle || productTitle.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || `bundle-${productId.slice(0, 8)}`;
      await onCreateBundle({
        bundleProductId: productId,
        slug,
        name: productTitle || 'Product Bundle',
        bundleType: 'fixed',
        status: 'active',
        discountType: 'none',
        discountValue: '0'
      });
    } catch (err) {
      console.error('Failed to create bundle:', err);
    } finally {
      setIsUpdating(false);
    }
  };

  const items = bundle?.items || [];
  const requiredCount = items.filter((i) => !i.isOptional).length;
  const optionalCount = items.filter((i) => i.isOptional).length;

  return (
    <Card className='overflow-hidden shadow-xs'>
      <CardHeader className='flex flex-row items-center justify-between border-b bg-muted/20 pb-4'>
        <div className='space-y-1'>
          <div className='flex items-center gap-2'>
            <Boxes className='h-5 w-5 text-primary' />
            <CardTitle className='text-base font-semibold'>
              Product Bundle & Kit Composition
            </CardTitle>
            {bundle && (
              <Badge variant='outline' className='gap-1 font-mono text-xs'>
                <Layers className='h-3 w-3' />
                {items.length} {items.length === 1 ? 'item' : 'items'}
              </Badge>
            )}
          </div>
          <CardDescription>
            Group products into fixed kits or configurable sets with package discounts.
          </CardDescription>
        </div>

        {bundle && (
          <div className='flex items-center gap-2'>
            {hasConfigChanges && (
              <Button
                size='sm'
                variant='outline'
                onClick={handleSaveConfig}
                disabled={isUpdating || disabled}
                className='text-xs'
              >
                Save Settings
              </Button>
            )}
            <Button
              size='sm'
              onClick={() => setIsItemDialogOpen(true)}
              disabled={isUpdating || disabled}
              className='gap-1 text-xs'
            >
              <Plus className='h-3.5 w-3.5' />
              Add Item
            </Button>
          </div>
        )}
      </CardHeader>

      <CardContent className='p-6'>
        {!bundle ? (
          <div className='flex flex-col items-center justify-center rounded-lg border-2 border-dashed border-muted p-8 text-center'>
            <div className='rounded-full bg-primary/10 p-3 text-primary mb-3'>
              <Sparkles className='h-6 w-6' />
            </div>
            <h4 className='text-base font-semibold'>Configure as Product Bundle</h4>
            <p className='mt-1 max-w-md text-sm text-muted-foreground'>
              Bundle multiple catalog items into a cohesive package. You can configure
              bundle-level percentage discounts, fixed reductions, and optional items.
            </p>
            <Button
              className='mt-4 gap-1.5'
              onClick={handleInitBundle}
              disabled={isUpdating || disabled}
            >
              <Boxes className='h-4 w-4' />
              Initialize Bundle Composition
            </Button>
          </div>
        ) : (
          <div className='space-y-6'>
            {/* Bundle Settings Grid */}
            <div className='grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 rounded-lg border bg-muted/10 p-4'>
              <div className='space-y-1.5'>
                <Label className='text-xs font-semibold uppercase tracking-wider text-muted-foreground'>
                  Kit Type
                </Label>
                <Select
                  value={bundleType}
                  onValueChange={setBundleType}
                  disabled={isUpdating || disabled}
                >
                  <SelectTrigger className='h-9 text-xs'>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value='fixed'>Fixed Composition</SelectItem>
                    <SelectItem value='flexible'>Flexible / Configurable</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <div className='space-y-1.5'>
                <Label className='text-xs font-semibold uppercase tracking-wider text-muted-foreground'>
                  Status
                </Label>
                <Select
                  value={status}
                  onValueChange={setStatus}
                  disabled={isUpdating || disabled}
                >
                  <SelectTrigger className='h-9 text-xs'>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value='active'>Active</SelectItem>
                    <SelectItem value='draft'>Draft</SelectItem>
                    <SelectItem value='archived'>Archived</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <div className='space-y-1.5'>
                <Label className='text-xs font-semibold uppercase tracking-wider text-muted-foreground'>
                  Bundle Discount
                </Label>
                <Select
                  value={discountType}
                  onValueChange={setDiscountType}
                  disabled={isUpdating || disabled}
                >
                  <SelectTrigger className='h-9 text-xs'>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value='none'>None</SelectItem>
                    <SelectItem value='percentage'>Percentage (%)</SelectItem>
                    <SelectItem value='fixed_amount'>Fixed Amount</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <div className='space-y-1.5'>
                <Label className='text-xs font-semibold uppercase tracking-wider text-muted-foreground'>
                  Discount Value
                </Label>
                <div className='relative'>
                  <Input
                    type='number'
                    min={0}
                    step={discountType === 'percentage' ? 0.5 : 0.01}
                    value={discountValue}
                    onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                      setDiscountValue(e.target.value)
                    }
                    disabled={discountType === 'none' || isUpdating || disabled}
                    className='h-9 text-xs pr-7'
                  />
                  {discountType === 'percentage' && (
                    <Percent className='absolute right-2.5 top-2.5 h-3.5 w-3.5 text-muted-foreground' />
                  )}
                </div>
              </div>
            </div>

            {/* Bundle Items Table */}
            <div className='rounded-md border'>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead className='w-[45%]'>Product Item</TableHead>
                    <TableHead className='text-center'>Qty</TableHead>
                    <TableHead>Component Role</TableHead>
                    <TableHead>Item Discount</TableHead>
                    <TableHead className='text-right'>Actions</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {items.length === 0 ? (
                    <TableRow>
                      <TableCell
                        colSpan={5}
                        className='h-28 text-center text-sm text-muted-foreground'
                      >
                        <div className='flex flex-col items-center justify-center gap-1.5'>
                          <Package className='h-6 w-6 text-muted-foreground/60' />
                          <span>No items in this bundle yet.</span>
                          <Button
                            variant='link'
                            size='sm'
                            className='h-auto p-0 text-xs'
                            onClick={() => setIsItemDialogOpen(true)}
                          >
                            Add the first bundle item
                          </Button>
                        </div>
                      </TableCell>
                    </TableRow>
                  ) : (
                    items.map((item) => {
                      const discountRateNum = item.discountRate
                        ? parseFloat(item.discountRate) * 100
                        : null;

                      return (
                        <TableRow key={item.id}>
                          <TableCell>
                            <div className='flex items-center gap-3'>
                              <div className='flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-muted'>
                                {item.product?.thumbnail ? (
                                  <img
                                    src={item.product.thumbnail}
                                    alt={item.product.title}
                                    className='h-full w-full rounded-md object-cover'
                                  />
                                ) : (
                                  <Package className='h-4 w-4 text-muted-foreground' />
                                )}
                              </div>
                              <div className='min-w-0'>
                                <p className='font-medium text-sm truncate'>
                                  {item.product?.title || item.productId}
                                </p>
                                <p className='text-xs text-muted-foreground font-mono truncate'>
                                  {item.variant?.sku
                                    ? `SKU: ${item.variant.sku}`
                                    : `ID: ${item.productId.slice(0, 8)}...`}
                                </p>
                              </div>
                            </div>
                          </TableCell>

                          <TableCell className='text-center font-medium'>
                            <span className='inline-flex h-6 min-w-6 items-center justify-center rounded bg-muted px-2 font-mono text-xs font-semibold'>
                              {item.quantity}x
                            </span>
                          </TableCell>

                          <TableCell>
                            {item.isOptional ? (
                              <Badge
                                variant='outline'
                                className='border-amber-500/30 bg-amber-500/10 text-amber-700 dark:text-amber-400 text-xs font-normal'
                              >
                                Optional add-on
                              </Badge>
                            ) : (
                              <Badge
                                variant='secondary'
                                className='gap-1 text-xs font-normal'
                              >
                                <CheckCircle2 className='h-3 w-3 text-primary' />
                                Required component
                              </Badge>
                            )}
                          </TableCell>

                          <TableCell>
                            {discountRateNum ? (
                              <span className='inline-flex items-center rounded-sm bg-emerald-500/10 px-1.5 py-0.5 font-mono text-xs font-medium text-emerald-700 dark:text-emerald-400'>
                                -{discountRateNum}%
                              </span>
                            ) : (
                              <span className='text-xs text-muted-foreground'>-</span>
                            )}
                          </TableCell>

                          <TableCell className='text-right'>
                            <Button
                              variant='ghost'
                              size='icon'
                              className='h-8 w-8 text-destructive hover:bg-destructive/10'
                              onClick={() =>
                                onRemoveBundleItem &&
                                onRemoveBundleItem(bundle.id, item.id)
                              }
                              disabled={isUpdating || disabled}
                            >
                              <Trash2 className='h-4 w-4' />
                            </Button>
                          </TableCell>
                        </TableRow>
                      );
                    })
                  )}
                </TableBody>
              </Table>
            </div>

            {/* Bundle Composition Summary Bar */}
            {items.length > 0 && (
              <div className='flex flex-wrap items-center justify-between gap-4 rounded-lg bg-muted/30 px-4 py-3 text-xs'>
                <div className='flex items-center gap-4'>
                  <div>
                    <span className='text-muted-foreground'>Total Items: </span>
                    <span className='font-semibold'>{items.length}</span>
                  </div>
                  <div>
                    <span className='text-muted-foreground'>Required: </span>
                    <span className='font-semibold text-primary'>{requiredCount}</span>
                  </div>
                  <div>
                    <span className='text-muted-foreground'>Optional: </span>
                    <span className='font-semibold text-amber-600 dark:text-amber-400'>
                      {optionalCount}
                    </span>
                  </div>
                </div>

                <div className='flex items-center gap-2'>
                  <span className='text-muted-foreground'>Package Discount:</span>
                  <span className='font-mono font-semibold'>
                    {discountType === 'percentage'
                      ? `${discountValue}% OFF`
                      : discountType === 'fixed_amount'
                      ? `${discountValue} OFF`
                      : 'None'}
                  </span>
                </div>
              </div>
            )}
          </div>
        )}

        {/* Add Item Dialog */}
        {bundle && (
          <ProductBundleItemDialog
            bundleId={bundle.id}
            open={isItemDialogOpen}
            onOpenChange={setIsItemDialogOpen}
            onAdd={async (input) => {
              if (onAddBundleItem) {
                await onAddBundleItem(input);
              }
            }}
            onSearchProducts={onSearchProducts}
            currentItemsCount={items.length}
            disabled={disabled}
          />
        )}
      </CardContent>
    </Card>
  );
}
