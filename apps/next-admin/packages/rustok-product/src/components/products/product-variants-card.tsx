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
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import { Badge } from '@/shared/ui/shadcn/badge';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from '@/shared/ui/shadcn/dialog';
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
} from '@/widgets/data-table/table';
import { Plus, Edit2, Trash2, Box, DollarSign } from 'lucide-react';
import type { ProductVariant } from '../../api/types';

interface ProductVariantsCardProps {
  variants: ProductVariant[];
  onAddVariant?: (variant: Omit<ProductVariant, 'id'>) => Promise<void>;
  onUpdateVariant?: (
    id: string,
    variant: Partial<ProductVariant>
  ) => Promise<void>;
  onDeleteVariant?: (id: string) => Promise<void>;
  isNew: boolean;
  onNewVariantDraftChange?: (draft: {
    sku: string;
    barcode: string;
    priceAmount: number;
    currencyCode: string;
    compareAtAmount?: number | null;
    inventoryQuantity: number;
    inventoryPolicy: string;
  }) => void;
  newVariantDraft?: {
    sku: string;
    barcode: string;
    priceAmount: number;
    currencyCode: string;
    compareAtAmount?: number | null;
    inventoryQuantity: number;
    inventoryPolicy: string;
  };
  disabled?: boolean;
}

export function ProductVariantsCard({
  variants,
  onAddVariant,
  onUpdateVariant,
  onDeleteVariant,
  isNew,
  onNewVariantDraftChange,
  newVariantDraft,
  disabled = false
}: ProductVariantsCardProps) {
  const [dialogOpen, setDialogOpen] = React.useState(false);
  const [editingVariant, setEditingVariant] =
    React.useState<ProductVariant | null>(null);
  const [isBusy, setIsBusy] = React.useState(false);

  // Dialog form fields
  const [formSku, setFormSku] = React.useState('');
  const [formBarcode, setFormBarcode] = React.useState('');
  const [formPrice, setFormPrice] = React.useState('0.00');
  const [formCurrency, setFormCurrency] = React.useState('USD');
  const [formCompareAt, setFormCompareAt] = React.useState('');
  const [formStock, setFormStock] = React.useState('0');
  const [formPolicy, setFormPolicy] = React.useState('deny');

  const openAddDialog = () => {
    setEditingVariant(null);
    setFormSku('');
    setFormBarcode('');
    setFormPrice('0.00');
    setFormCurrency('USD');
    setFormCompareAt('');
    setFormStock('0');
    setFormPolicy('deny');
    setDialogOpen(true);
  };

  const openEditDialog = (variant: ProductVariant) => {
    setEditingVariant(variant);
    setFormSku(variant.sku || '');
    setFormBarcode(variant.barcode || '');
    const primaryPrice = variant.prices[0];
    setFormPrice(
      primaryPrice ? (primaryPrice.amount / 100).toFixed(2) : '0.00'
    );
    setFormCurrency(primaryPrice ? primaryPrice.currencyCode : 'USD');
    setFormCompareAt(
      primaryPrice?.compareAtAmount
        ? (primaryPrice.compareAtAmount / 100).toFixed(2)
        : ''
    );
    setFormStock(variant.inventoryQuantity.toString());
    setFormPolicy(variant.inventoryPolicy || 'deny');
    setDialogOpen(true);
  };

  const handleSaveDialog = async () => {
    const priceRaw = parseFloat(formPrice);
    const amount =
      Number.isFinite(priceRaw) && priceRaw > 0
        ? Math.round(priceRaw * 100)
        : 0;
    const compareRaw = parseFloat(formCompareAt);
    const compareAtAmount =
      Number.isFinite(compareRaw) && compareRaw > 0
        ? Math.round(compareRaw * 100)
        : null;
    const qty = parseInt(formStock, 10) || 0;

    setIsBusy(true);
    try {
      if (editingVariant && onUpdateVariant) {
        await onUpdateVariant(editingVariant.id, {
          sku: formSku || null,
          barcode: formBarcode || null,
          inventoryQuantity: qty,
          inventoryPolicy: formPolicy,
          prices: [
            {
              currencyCode: formCurrency,
              amount,
              compareAtAmount,
              onSale: Boolean(compareAtAmount && compareAtAmount > amount)
            }
          ]
        });
      } else if (onAddVariant) {
        await onAddVariant({
          sku: formSku || null,
          barcode: formBarcode || null,
          title: formSku ? `Variant ${formSku}` : null,
          inventoryQuantity: qty,
          inventoryPolicy: formPolicy,
          inStock: qty > 0,
          prices: [
            {
              currencyCode: formCurrency,
              amount,
              compareAtAmount,
              onSale: Boolean(compareAtAmount && compareAtAmount > amount)
            }
          ]
        });
      }
      setDialogOpen(false);
    } finally {
      setIsBusy(false);
    }
  };

  const formatPrice = (amount: number, currency: string) => {
    return (amount / 100).toLocaleString(undefined, {
      style: 'currency',
      currency: currency || 'USD'
    });
  };

  return (
    <Card className='border-border rounded-2xl shadow-sm'>
      <CardHeader className='border-border/60 border-b pb-4'>
        <div className='flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between'>
          <div className='flex items-center gap-2'>
            <Box className='text-primary h-4 w-4' />
            <div>
              <CardTitle className='text-sm font-semibold'>
                Pricing & Variants
              </CardTitle>
              <CardDescription className='text-xs'>
                Manage SKUs, barcodes, multi-currency pricing, and inventory
                quantities.
              </CardDescription>
            </div>
          </div>
          {!isNew && onAddVariant && (
            <Button
              type='button'
              size='sm'
              onClick={openAddDialog}
              className='h-8 gap-1.5 self-start rounded-xl px-3 text-xs sm:self-auto'
              disabled={disabled}
            >
              <Plus className='h-3.5 w-3.5' />
              <span>Add Variant</span>
            </Button>
          )}
        </div>
      </CardHeader>
      <CardContent className='space-y-4 pt-5'>
        {isNew ? (
          // Simple inline variant inputs for new products
          <div className='grid grid-cols-1 gap-4 sm:grid-cols-2 md:grid-cols-3'>
            <div className='space-y-1.5'>
              <Label htmlFor='new-price' className='text-xs font-medium'>
                Price Amount <span className='text-destructive'>*</span>
              </Label>
              <div className='border-input focus-within:ring-ring bg-background flex overflow-hidden rounded-xl border focus-within:ring-1'>
                <span className='text-muted-foreground bg-muted/40 border-border border-r px-3 py-2 text-xs select-none'>
                  {newVariantDraft?.currencyCode || 'USD'}
                </span>
                <input
                  id='new-price'
                  type='number'
                  step='0.01'
                  value={
                    newVariantDraft
                      ? (newVariantDraft.priceAmount / 100).toFixed(2)
                      : '0.00'
                  }
                  onChange={(e) => {
                    const parsed = parseFloat(e.target.value);
                    const amount =
                      Number.isFinite(parsed) && parsed > 0
                        ? Math.round(parsed * 100)
                        : 0;
                    onNewVariantDraftChange?.({
                      ...(newVariantDraft || {
                        sku: '',
                        barcode: '',
                        currencyCode: 'USD',
                        inventoryQuantity: 0,
                        inventoryPolicy: 'deny'
                      }),
                      priceAmount: amount
                    });
                  }}
                  className='flex-1 bg-transparent px-3 py-2 font-mono text-xs outline-none'
                  disabled={disabled}
                />
              </div>
            </div>

            <div className='space-y-1.5'>
              <Label htmlFor='new-sku' className='text-xs font-medium'>
                Primary SKU
              </Label>
              <Input
                id='new-sku'
                value={newVariantDraft?.sku || ''}
                onChange={(e) =>
                  onNewVariantDraftChange?.({
                    ...(newVariantDraft || {
                      priceAmount: 0,
                      currencyCode: 'USD',
                      barcode: '',
                      inventoryQuantity: 0,
                      inventoryPolicy: 'deny'
                    }),
                    sku: e.target.value
                  })
                }
                placeholder='e.g. PRD-001'
                className='h-9 rounded-xl font-mono text-xs'
                disabled={disabled}
              />
            </div>

            <div className='space-y-1.5'>
              <Label htmlFor='new-stock' className='text-xs font-medium'>
                Inventory Stock
              </Label>
              <Input
                id='new-stock'
                type='number'
                value={newVariantDraft?.inventoryQuantity ?? 0}
                onChange={(e) => {
                  const qty = parseInt(e.target.value, 10) || 0;
                  onNewVariantDraftChange?.({
                    ...(newVariantDraft || {
                      sku: '',
                      barcode: '',
                      priceAmount: 0,
                      currencyCode: 'USD',
                      inventoryPolicy: 'deny'
                    }),
                    inventoryQuantity: qty
                  });
                }}
                className='h-9 rounded-xl font-mono text-xs'
                disabled={disabled}
              />
            </div>
          </div>
        ) : variants.length === 0 ? (
          <div className='text-muted-foreground bg-muted/20 rounded-xl py-8 text-center text-xs italic'>
            No variants found for this product. Click "Add Variant" to create
            one.
          </div>
        ) : (
          <div className='border-border overflow-hidden rounded-xl border'>
            <Table>
              <TableHeader className='bg-muted/50'>
                <TableRow className='text-[11px]'>
                  <TableHead>SKU / Barcode</TableHead>
                  <TableHead>Price</TableHead>
                  <TableHead>Compare-at</TableHead>
                  <TableHead>Stock</TableHead>
                  <TableHead>Policy</TableHead>
                  <TableHead className='text-right'>Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody className='text-xs'>
                {variants.map((v) => {
                  const price = v.prices[0];
                  return (
                    <TableRow key={v.id} className='hover:bg-accent/40'>
                      <TableCell className='font-mono'>
                        <div className='text-foreground font-medium'>
                          {v.sku || '—'}
                        </div>
                        {v.barcode && (
                          <div className='text-muted-foreground text-[10px]'>
                            {v.barcode}
                          </div>
                        )}
                      </TableCell>
                      <TableCell className='text-foreground font-semibold'>
                        {price
                          ? formatPrice(price.amount, price.currencyCode)
                          : '—'}
                      </TableCell>
                      <TableCell className='text-muted-foreground text-[11px] line-through'>
                        {price?.compareAtAmount
                          ? formatPrice(
                              price.compareAtAmount,
                              price.currencyCode
                            )
                          : '—'}
                      </TableCell>
                      <TableCell>
                        <Badge
                          variant={v.inStock ? 'secondary' : 'outline'}
                          className={`text-[10px] ${
                            v.inStock
                              ? 'border-emerald-500/20 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
                              : 'border-rose-500/30 text-rose-500'
                          }`}
                        >
                          {v.inventoryQuantity} in stock
                        </Badge>
                      </TableCell>
                      <TableCell className='text-muted-foreground font-mono text-[10px] uppercase'>
                        {v.inventoryPolicy}
                      </TableCell>
                      <TableCell className='text-right'>
                        <div className='flex items-center justify-end gap-1'>
                          <Button
                            type='button'
                            variant='ghost'
                            size='icon'
                            onClick={() => openEditDialog(v)}
                            className='text-muted-foreground hover:text-foreground h-7 w-7 rounded-lg'
                            disabled={disabled}
                          >
                            <Edit2 className='h-3.5 w-3.5' />
                          </Button>
                          {variants.length > 1 && onDeleteVariant && (
                            <Button
                              type='button'
                              variant='ghost'
                              size='icon'
                              onClick={() => onDeleteVariant(v.id)}
                              className='h-7 w-7 rounded-lg text-rose-500 hover:bg-rose-50 hover:text-rose-700 dark:hover:bg-rose-950/40'
                              disabled={disabled}
                            >
                              <Trash2 className='h-3.5 w-3.5' />
                            </Button>
                          )}
                        </div>
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </div>
        )}
      </CardContent>

      {/* Add / Edit Variant Dialog */}
      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className='rounded-2xl sm:max-w-md'>
          <DialogHeader>
            <DialogTitle className='text-sm font-semibold'>
              {editingVariant ? 'Edit Variant' : 'Add New Variant'}
            </DialogTitle>
            <DialogDescription className='text-xs'>
              Configure SKU, barcode identifier, price and inventory settings.
            </DialogDescription>
          </DialogHeader>

          <div className='space-y-4 py-2 text-xs'>
            <div className='grid grid-cols-2 gap-3'>
              <div className='space-y-1.5'>
                <Label htmlFor='var-sku'>SKU</Label>
                <Input
                  id='var-sku'
                  value={formSku}
                  onChange={(e) => setFormSku(e.target.value)}
                  placeholder='e.g. MOUSE-BLK'
                  className='h-9 rounded-xl font-mono text-xs'
                />
              </div>
              <div className='space-y-1.5'>
                <Label htmlFor='var-barcode'>Barcode</Label>
                <Input
                  id='var-barcode'
                  value={formBarcode}
                  onChange={(e) => setFormBarcode(e.target.value)}
                  placeholder='e.g. 123456789012'
                  className='h-9 rounded-xl font-mono text-xs'
                />
              </div>
            </div>

            <div className='grid grid-cols-3 gap-3'>
              <div className='space-y-1.5'>
                <Label htmlFor='var-currency'>Currency</Label>
                <Select value={formCurrency} onValueChange={setFormCurrency}>
                  <SelectTrigger
                    id='var-currency'
                    className='h-9 rounded-xl font-mono text-xs'
                  >
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value='USD'>USD ($)</SelectItem>
                    <SelectItem value='EUR'>EUR (€)</SelectItem>
                    <SelectItem value='RUB'>RUB (₽)</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div className='space-y-1.5'>
                <Label htmlFor='var-price'>Price *</Label>
                <Input
                  id='var-price'
                  type='number'
                  step='0.01'
                  value={formPrice}
                  onChange={(e) => setFormPrice(e.target.value)}
                  className='h-9 rounded-xl font-mono text-xs'
                />
              </div>
              <div className='space-y-1.5'>
                <Label htmlFor='var-compare'>Compare-at</Label>
                <Input
                  id='var-compare'
                  type='number'
                  step='0.01'
                  value={formCompareAt}
                  onChange={(e) => setFormCompareAt(e.target.value)}
                  placeholder='0.00'
                  className='h-9 rounded-xl font-mono text-xs'
                />
              </div>
            </div>

            <div className='grid grid-cols-2 gap-3'>
              <div className='space-y-1.5'>
                <Label htmlFor='var-stock'>Stock Quantity</Label>
                <Input
                  id='var-stock'
                  type='number'
                  value={formStock}
                  onChange={(e) => setFormStock(e.target.value)}
                  className='h-9 rounded-xl font-mono text-xs'
                />
              </div>
              <div className='space-y-1.5'>
                <Label htmlFor='var-policy'>Inventory Policy</Label>
                <Select value={formPolicy} onValueChange={setFormPolicy}>
                  <SelectTrigger
                    id='var-policy'
                    className='h-9 rounded-xl text-xs'
                  >
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value='deny' className='text-xs'>
                      Deny on out of stock
                    </SelectItem>
                    <SelectItem value='continue' className='text-xs'>
                      Continue selling
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>
          </div>

          <DialogFooter className='gap-2 sm:gap-0'>
            <Button
              type='button'
              variant='outline'
              onClick={() => setDialogOpen(false)}
              className='h-9 rounded-xl text-xs'
            >
              Cancel
            </Button>
            <Button
              type='button'
              onClick={handleSaveDialog}
              disabled={isBusy}
              className='h-9 rounded-xl text-xs font-semibold'
            >
              {isBusy
                ? 'Saving...'
                : editingVariant
                  ? 'Update Variant'
                  : 'Create Variant'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Card>
  );
}
