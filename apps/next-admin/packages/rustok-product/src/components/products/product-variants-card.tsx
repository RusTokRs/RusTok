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

import type {
  CreateVariantInput,
  UpdateVariantInput
} from '../../api/products';
import * as React from 'react';
import { useTranslations } from '@rustok/next-fluent/client';
import { CombinationPicker } from './combination-picker';
import {
  MAX_COMBINATION_MEMBERS,
  savedCombinationAxes,
  retainedCombinationLabels
} from '../../combinations';
import {
  buildVariantUpdatePatch,
  validateAxisValues
} from '../../variant-draft';
import type { VariantAxisConfig, VariantAxisValueInput } from '../../api/types';
import { parsePriceAmount, salePercent, discountedPrice } from '../../money';
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
import {
  Plus,
  Edit2,
  Trash2,
  Box,
  DollarSign,
  Percent,
  Tag,
  Globe,
  Coins
} from 'lucide-react';
import type { ProductVariant, ProductVariantPrice } from '../../api/types';
import { MAX_INVENTORY_QUANTITY } from '../../limits';

interface PriceRowDraft {
  currencyCode: string;
  amount: string;
  compareAtAmount: string;
}

const SUPPORTED_CURRENCIES = [
  { code: 'USD', symbol: '$', label: 'USD ($)' },
  { code: 'EUR', symbol: '€', label: 'EUR (€)' },
  { code: 'RUB', symbol: '₽', label: 'RUB (₽)' },
  { code: 'GBP', symbol: '£', label: 'GBP (£)' },
  { code: 'CNY', symbol: '¥', label: 'CNY (¥)' },
  { code: 'KZT', symbol: '₸', label: 'KZT (₸)' },
  { code: 'BYN', symbol: 'Br', label: 'BYN (Br)' },
  { code: 'TRY', symbol: '₺', label: 'TRY (₺)' }
];

interface ProductVariantsCardProps {
  variants: ProductVariant[];
  axes?: VariantAxisConfig[];
  onAddVariant?: (variant: CreateVariantInput) => Promise<void>;
  onUpdateVariant?: (id: string, variant: UpdateVariantInput) => Promise<void>;
  onDeleteVariant?: (id: string, expectedRevision: string) => Promise<void>;
  /** An explicit, terminally rejected creation reopened as a new operation. */
  creationDraft?: { input: CreateVariantInput; intent: string } | null;
  disabled?: boolean;
}

export function ProductVariantsCard({
  variants,
  axes = [],
  onAddVariant,
  onUpdateVariant,
  onDeleteVariant,
  creationDraft,
  disabled = false
}: ProductVariantsCardProps) {
  const combinationsCopy = useTranslations('product.combinations');
  const creationCopy = useTranslations('product.variantCreate');
  const combinationAxes = React.useCallback(
    () => savedCombinationAxes(axes),
    [axes]
  );
  const [dialogOpen, setDialogOpen] = React.useState(false);
  const [editingVariant, setEditingVariant] =
    React.useState<ProductVariant | null>(null);
  const [isBusy, setIsBusy] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  // Dialog form fields
  const [formSku, setFormSku] = React.useState('');
  const [formAxisValues, setFormAxisValues] = React.useState<
    VariantAxisValueInput[]
  >([]);
  const [formBarcode, setFormBarcode] = React.useState('');
  const [formShippingProfile, setFormShippingProfile] = React.useState('');
  const [formPrices, setFormPrices] = React.useState<PriceRowDraft[]>([]);
  const [formStock, setFormStock] = React.useState('0');
  const [formPolicy, setFormPolicy] = React.useState('deny');

  const openAddDialog = () => {
    setError(null);
    setEditingVariant(null);
    setFormSku('');
    setFormAxisValues([]);
    setFormBarcode('');
    setFormShippingProfile('');
    setFormPrices([]);
    setFormStock('0');
    setFormPolicy('deny');
    setDialogOpen(true);
  };

  const restoredCreation = React.useRef<string | null>(null);
  React.useEffect(() => {
    if (
      !creationDraft ||
      disabled ||
      restoredCreation.current === creationDraft.intent
    )
      return;
    const input = creationDraft.input;
    restoredCreation.current = creationDraft.intent;
    setError(null);
    setEditingVariant(null);
    setFormSku(input.sku ?? '');
    setFormBarcode(input.barcode ?? '');
    setFormShippingProfile(input.shippingProfileSlug ?? '');
    setFormAxisValues(structuredClone(input.axisValues ?? []));
    setFormPrices(
      input.prices.map((price) => ({
        currencyCode: price.currencyCode,
        amount: price.amount,
        compareAtAmount: price.compareAtAmount ?? ''
      }))
    );
    setFormStock(String(input.inventoryQuantity ?? 0));
    setFormPolicy(input.inventoryPolicy ?? 'deny');
    setDialogOpen(true);
  }, [creationDraft, disabled]);

  const openEditDialog = (variant: ProductVariant) => {
    setError(null);
    setEditingVariant(variant);
    setFormSku(variant.sku || '');
    setFormAxisValues(variant.axisValues);
    setFormBarcode(variant.barcode || '');
    setFormShippingProfile(variant.shippingProfileSlug ?? '');

    if (variant.prices && variant.prices.length > 0) {
      setFormPrices(
        variant.prices.map((p) => ({
          currencyCode: p.currencyCode,
          amount: p.amount,
          compareAtAmount: p.compareAtAmount ?? ''
        }))
      );
    } else {
      setFormPrices([]);
    }

    setFormStock(variant.inventoryQuantity.toString());
    setFormPolicy(variant.inventoryPolicy || 'deny');
    setDialogOpen(true);
  };

  const handlePriceChange = (
    index: number,
    field: keyof PriceRowDraft,
    value: string
  ) => {
    setFormPrices((prev) => {
      const next = [...prev];
      next[index] = { ...next[index], [field]: value };
      return next;
    });
  };

  const handleApplyDiscountPercent = (index: number, percent: number) => {
    try {
      const row = formPrices[index];
      const base = parsePriceAmount(row.compareAtAmount || row.amount);
      const amount = discountedPrice(base, percent);
      setFormPrices((prev) =>
        prev.map((price, i) =>
          i === index ? { ...price, compareAtAmount: base, amount } : price
        )
      );
      setError(null);
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : 'Invalid price.');
    }
  };

  const handleAddPriceRow = () => {
    const existingCurrencies = new Set(formPrices.map((p) => p.currencyCode));
    const nextCur =
      SUPPORTED_CURRENCIES.find((c) => !existingCurrencies.has(c.code))?.code ||
      'EUR';
    setFormPrices((prev) => [
      ...prev,
      { currencyCode: nextCur, amount: '', compareAtAmount: '' }
    ]);
  };

  const handleRemovePriceRow = (index: number) => {
    setFormPrices((prev) => prev.filter((_, i) => i !== index));
  };

  const handleQuickInventoryUpdate = async (
    variantId: string,
    inventoryQuantity: number
  ) => {
    if (disabled || isBusy || !onUpdateVariant) return;
    const variant = variants.find((candidate) => candidate.id === variantId);
    if (!variant) {
      setError('The variant changed. Refresh the product and try again.');
      return;
    }
    setIsBusy(true);
    setError(null);
    try {
      await onUpdateVariant(variantId, {
        inventoryQuantity,
        expectedInventoryQuantity: variant.inventoryQuantity,
        expectedRevision: variant.revision
      });
    } catch (failure) {
      setError(
        failure instanceof Error ? failure.message : 'Failed to update stock.'
      );
    } finally {
      setIsBusy(false);
    }
  };

  const handleDeleteVariant = async (variantId: string) => {
    if (disabled || isBusy || !onDeleteVariant) return;
    if (
      !window.confirm(
        combinationsCopy('deleteConfirm')
      )
    )
      return;
    const variant = variants.find((candidate) => candidate.id === variantId);
    if (!variant) {
      setError('The variant changed. Refresh the product and try again.');
      return;
    }
    setIsBusy(true);
    setError(null);
    try {
      await onDeleteVariant(variantId, variant.revision);
    } catch (failure) {
      setError(
        failure instanceof Error ? failure.message : 'Failed to delete variant.'
      );
    } finally {
      setIsBusy(false);
    }
  };

  const handleSaveDialog = async () => {
    if (disabled || isBusy) return;
    if (editingVariant ? !onUpdateVariant : !onAddVariant) {
      setError(combinationsCopy('writeUnavailable'));
      return;
    }

    const qty = Number(formStock);
    const inventoryInputChanged =
      !editingVariant || formStock !== String(editingVariant.inventoryQuantity);
    if (
      inventoryInputChanged &&
      (!/^\d+$/.test(formStock) ||
        !Number.isInteger(qty) ||
        qty > MAX_INVENTORY_QUANTITY)
    ) {
      setError('Inventory must be a non-negative 32-bit integer.');
      return;
    }

    let mappedPrices: ProductVariantPrice[] = [];
    let updatePatch: UpdateVariantInput | null = null;
    try {
      // Scalar/price editing does not submit assignments. A stale dialog tuple
      // after an axis refresh must not block that unrelated edit.
      if (!editingVariant) {
        validateAxisValues(
          axes.map((axis) => ({
            attributeId: axis.attributeId,
            allowedOptionIds: axis.allowedValues.map((value) => value.optionId)
          })),
          formAxisValues
        );
        mappedPrices = formPrices.map((price) => ({
          currencyCode: price.currencyCode,
          amount: parsePriceAmount(price.amount),
          compareAtAmount: price.compareAtAmount.trim()
            ? parsePriceAmount(price.compareAtAmount)
            : null
        }));
      } else {
        // Compare raw form values first. Parse only a changed price set so a
        // SKU-only edit cannot accidentally replay or normalize stale prices.
        updatePatch = buildVariantUpdatePatch(editingVariant, {
          sku: formSku,
          barcode: formBarcode,
          shippingProfileSlug: formShippingProfile,
          inventoryQuantity: qty,
          inventoryPolicy: formPolicy,
          prices: formPrices
        });
        if (updatePatch.prices) {
          updatePatch.prices = updatePatch.prices.map((price) => ({
            ...price,
            amount: parsePriceAmount(price.amount),
            compareAtAmount: price.compareAtAmount?.trim()
              ? parsePriceAmount(price.compareAtAmount)
              : null
          }));
        }
      }
      setError(null);
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : 'Invalid price.');
      return;
    }

    setIsBusy(true);
    try {
      if (editingVariant) {
        const patch = updatePatch ?? {};
        if (Object.keys(patch).length === 0) {
          setDialogOpen(false);
          return;
        }
        await onUpdateVariant!(editingVariant.id, patch);
      } else {
        await onAddVariant!({
          sku: formSku || null,
          barcode: formBarcode || null,
          inventoryQuantity: qty,
          inventoryPolicy: formPolicy,
          prices: mappedPrices,
          shippingProfileSlug: formShippingProfile || null,
          axisValues: formAxisValues
        });
      }
      setDialogOpen(false);
    } catch (failure) {
      setError(
        failure instanceof Error ? failure.message : 'Failed to save variant.'
      );
    } finally {
      setIsBusy(false);
    }
  };

  const formatPrice = (amount: string, currency: string) =>
    `${amount} ${currency}`;

  return (
    <Card className='border-border rounded-2xl shadow-sm'>
      <CardHeader className='border-border/60 border-b pb-4'>
        <div className='flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between'>
          <div className='flex items-center gap-2'>
            <Box className='text-primary h-4 w-4' />
            <div>
              <CardTitle className='text-sm font-semibold'>
                Pricing & Variants Matrix
              </CardTitle>
              <CardDescription className='text-xs'>
                SKUs, barcodes, the multi-currency price grid, discounts, and stock
              </CardDescription>
            </div>
          </div>
          {onAddVariant && (
            <Button
              type='button'
              size='sm'
              onClick={openAddDialog}
              className='h-8 gap-1.5 self-start rounded-xl px-3 text-xs sm:self-auto'
              disabled={
                disabled ||
                variants.length >= MAX_COMBINATION_MEMBERS ||
                (axes.length === 0 && variants.length > 0)
              }
            >
              <Plus className='h-3.5 w-3.5' />
              Add Variant
            </Button>
          )}
        </div>
      </CardHeader>
      <CardContent className='pt-4'>
        {error && !dialogOpen ? (
          <p role='alert' className='text-destructive mb-3 text-sm'>
            {error}
          </p>
        ) : null}
        {variants.length === 0 ? (
          <div className='text-muted-foreground bg-muted/20 rounded-xl py-8 text-center text-xs italic'>
            No variants for this product yet. Click "Add Variant".
          </div>
        ) : (
          <div className='border-border overflow-hidden rounded-xl border'>
            <Table>
              <TableHeader className='bg-muted/50'>
                <TableRow className='text-[11px]'>
                  <TableHead>SKU / Barcode</TableHead>
                  <TableHead>{combinationsCopy('assignments')}</TableHead>
                  <TableHead>Base Price</TableHead>
                  <TableHead>Currency Grid</TableHead>
                  <TableHead>Stock</TableHead>
                  <TableHead>Policy</TableHead>
                  <TableHead className='text-right'>Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody className='text-xs'>
                {variants.map((v) => {
                  const primaryPrice = v.prices[0];
                  const additionalPrices = v.prices.slice(1);
                  const discountPct = primaryPrice
                    ? salePercent(
                        primaryPrice.amount,
                        primaryPrice.compareAtAmount
                      )
                    : null;
                  const isOnSale = discountPct !== null;

                  return (
                    <TableRow key={v.id} className='hover:bg-accent/40 group'>
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

                      <TableCell>
                        {v.axisValues.length
                          ? retainedCombinationLabels(axes, v.axisValues).join(
                              ' · '
                            )
                          : axes.length
                            ? combinationsCopy('unassigned')
                            : combinationsCopy('default')}
                      </TableCell>

                      {/* Primary Price & Sale Badges */}
                      <TableCell>
                        {primaryPrice ? (
                          <div className='space-y-0.5'>
                            <div className='flex items-center gap-1.5'>
                              <span className='text-foreground font-semibold'>
                                {formatPrice(
                                  primaryPrice.amount,
                                  primaryPrice.currencyCode
                                )}
                              </span>
                              {isOnSale && (
                                <Badge
                                  variant='outline'
                                  className='border-rose-200 bg-rose-50 px-1 py-0 text-[9px] text-rose-700 dark:bg-rose-950/40 dark:text-rose-300'
                                >
                                  -{discountPct}%
                                </Badge>
                              )}
                            </div>
                            {primaryPrice.compareAtAmount && (
                              <div className='text-muted-foreground text-[10px] line-through'>
                                {formatPrice(
                                  primaryPrice.compareAtAmount,
                                  primaryPrice.currencyCode
                                )}
                              </div>
                            )}
                          </div>
                        ) : (
                          '—'
                        )}
                      </TableCell>

                      {/* Additional Currency Prices */}
                      <TableCell>
                        {additionalPrices.length > 0 ? (
                          <div className='flex flex-wrap gap-1'>
                            {additionalPrices.map((p, pIdx) => (
                              <Badge
                                key={pIdx}
                                variant='secondary'
                                className='font-mono text-[10px] font-normal'
                              >
                                {formatPrice(p.amount, p.currencyCode)}
                              </Badge>
                            ))}
                          </div>
                        ) : (
                          <span className='text-muted-foreground text-[11px]'>
                            Single currency
                          </span>
                        )}
                      </TableCell>

                      {/* Stock & Quick Adjust */}
                      <TableCell>
                        <div className='flex items-center gap-1.5'>
                          <Badge
                            variant='outline'
                            className={`font-mono text-[10px] font-medium ${
                              v.inventoryQuantity <= 0
                                ? 'border-rose-200 bg-rose-50 text-rose-700 dark:bg-rose-950/40 dark:text-rose-300'
                                : v.inventoryQuantity <= 5
                                  ? 'border-amber-200 bg-amber-50 text-amber-700 dark:bg-amber-950/40 dark:text-amber-300'
                                  : 'border-emerald-200 bg-emerald-50 text-emerald-700 dark:bg-emerald-950/40 dark:text-emerald-300'
                            }`}
                          >
                            {v.inventoryQuantity} pcs
                            {v.inventoryQuantity <= 0
                              ? ' (0)'
                              : v.inventoryQuantity <= 5
                                ? ' (Low)'
                                : ''}
                          </Badge>
                          {onUpdateVariant && (
                            <div className='flex items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100'>
                              <button
                                type='button'
                                onClick={() =>
                                  void handleQuickInventoryUpdate(
                                    v.id,
                                    Math.max(0, v.inventoryQuantity - 1)
                                  )
                                }
                                disabled={disabled || isBusy || v.inventoryQuantity <= 0}
                                className='bg-muted hover:bg-muted/80 text-muted-foreground hover:text-foreground flex h-5 w-5 items-center justify-center rounded text-[11px] font-bold'
                                title='Decrease stock by 1'
                              >
                                -
                              </button>
                              <button
                                type='button'
                                onClick={() =>
                                  void handleQuickInventoryUpdate(
                                    v.id,
                                    v.inventoryQuantity + 1
                                  )
                                }
                                disabled={disabled || isBusy || v.inventoryQuantity >= MAX_INVENTORY_QUANTITY}
                                className='bg-muted hover:bg-muted/80 text-muted-foreground hover:text-foreground flex h-5 w-5 items-center justify-center rounded text-[11px] font-bold'
                                title='Increase stock by 1'
                              >
                                +
                              </button>
                            </div>
                          )}
                        </div>
                      </TableCell>

                      {/* Policy */}
                      <TableCell className='text-muted-foreground font-mono text-[11px]'>
                        {v.inventoryPolicy}
                      </TableCell>

                      {/* Actions */}
                      <TableCell className='text-right'>
                        <div className='flex items-center justify-end gap-1'>
                          {onUpdateVariant && (
                            <Button
                              type='button'
                              variant='ghost'
                              size='icon'
                              onClick={() => openEditDialog(v)}
                              className='text-muted-foreground hover:text-foreground h-7 w-7 rounded-lg'
                              disabled={disabled || isBusy}
                              title='Edit variant and prices'
                            >
                              <Edit2 className='h-3.5 w-3.5' />
                            </Button>
                          )}
                          {onDeleteVariant && variants.length > 1 && (
                            <Button
                              type='button'
                              variant='ghost'
                              size='icon'
                              onClick={() => void handleDeleteVariant(v.id)}
                              className='h-7 w-7 rounded-lg text-rose-500 hover:bg-rose-50 hover:text-rose-700 dark:hover:bg-rose-950/40'
                              disabled={disabled || isBusy}
                              title='Delete variant'
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

      {/* Add / Edit Variant Dialog with Multi-Currency & Discount Tools */}
      <Dialog
        open={dialogOpen}
        onOpenChange={(open) => {
          if (!isBusy) setDialogOpen(open);
        }}
      >
        <DialogContent className='max-h-[90vh] overflow-y-auto rounded-2xl sm:max-w-lg'>
          <DialogHeader>
            <DialogTitle className='text-sm font-semibold'>
              {editingVariant
                ? 'Edit Variant'
                : 'New Product Variant'}
            </DialogTitle>
            <DialogDescription className='text-xs'>
              Set the SKU, barcode, multi-currency price grid, and discount
              rules.
            </DialogDescription>
          </DialogHeader>

          <fieldset disabled={disabled || isBusy} className='min-w-0 space-y-3'>
            {!editingVariant && (
              <CombinationPicker
                getAxes={combinationAxes}
                members={variants}
                disabled={disabled || isBusy || !onAddVariant}
                atCapacity={variants.length >= MAX_COMBINATION_MEMBERS}
                onPick={(axisValues) => {
                  if (!disabled && !isBusy) setFormAxisValues(axisValues);
                }}
              />
            )}
            <div className='space-y-4 py-2 text-xs'>
              {/* SKU and Barcode */}
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
                  <Label htmlFor='var-barcode'>Barcode (EAN)</Label>
                  <Input
                    id='var-barcode'
                    value={formBarcode}
                    onChange={(e) => setFormBarcode(e.target.value)}
                    placeholder='e.g. 123456789012'
                    className='h-9 rounded-xl font-mono text-xs'
                  />
                </div>
              </div>

              <div className='space-y-1.5'>
                <Label htmlFor='var-shipping-profile'>
                  {creationCopy('shippingProfile')}
                </Label>
                <Input
                  id='var-shipping-profile'
                  maxLength={100}
                  value={formShippingProfile}
                  onChange={(event) =>
                    setFormShippingProfile(event.target.value)
                  }
                />
              </div>
              {!editingVariant && formAxisValues.length > 0 && (
                <div className='space-y-2'>
                  <p>
                    {combinationsCopy('assignments')}:{' '}
                    {retainedCombinationLabels(axes, formAxisValues).join(
                      ' · '
                    )}
                  </p>
                  <Button
                    type='button'
                    variant='outline'
                    onClick={() => setFormAxisValues([])}
                  >
                    {creationCopy('clearAssignments')}
                  </Button>
                </div>
              )}
              {/* Pricing Section - Multi-Currency Grid */}
              <div className='border-border bg-muted/10 space-y-3 rounded-xl border p-3.5'>
                <div className='flex items-center justify-between'>
                  <div className='text-foreground flex items-center gap-1.5 text-xs font-semibold'>
                    <Coins className='text-primary h-3.5 w-3.5' />
                    <span>Currency Grid & Prices</span>
                  </div>
                  <Button
                    type='button'
                    variant='outline'
                    size='sm'
                    onClick={handleAddPriceRow}
                    className='h-7 gap-1 rounded-lg px-2 text-[11px]'
                  >
                    <Plus className='h-3 w-3' />
                    Add Currency
                  </Button>
                </div>

                {!editingVariant && formPrices.length === 0 && (
                  <p className='text-muted-foreground text-xs'>
                    {combinationsCopy('noPrice')}
                  </p>
                )}
                {/* Price Rows */}
                <div className='space-y-2.5'>
                  {formPrices.map((row, idx) => {
                    let discountPct: string | null = null;
                    try {
                      discountPct = salePercent(
                        row.amount,
                        row.compareAtAmount
                      );
                    } catch {
                      /* Incomplete input is validated on save. */
                    }
                    const hasDiscount = discountPct !== null;

                    return (
                      <div
                        key={idx}
                        className='border-border/80 bg-background space-y-2 rounded-xl border p-2.5'
                      >
                        <div className='grid grid-cols-3 items-end gap-2.5'>
                          {/* Currency */}
                          <div className='space-y-1'>
                            <Label className='text-muted-foreground text-[10px]'>
                              Currency
                            </Label>
                            <Select
                              disabled={disabled || isBusy}
                              value={row.currencyCode}
                              onValueChange={(val) =>
                                handlePriceChange(idx, 'currencyCode', val)
                              }
                            >
                              <SelectTrigger className='h-8 rounded-lg font-mono text-xs'>
                                <SelectValue />
                              </SelectTrigger>
                              <SelectContent>
                                {!SUPPORTED_CURRENCIES.some(
                                  (currency) =>
                                    currency.code === row.currencyCode
                                ) && row.currencyCode ? (
                                  <SelectItem value={row.currencyCode}>
                                    {row.currencyCode}
                                  </SelectItem>
                                ) : null}
                                {SUPPORTED_CURRENCIES.map((c) => (
                                  <SelectItem
                                    key={c.code}
                                    value={c.code}
                                    className='text-xs'
                                  >
                                    {c.label}
                                  </SelectItem>
                                ))}
                              </SelectContent>
                            </Select>
                          </div>

                          {/* Price */}
                          <div className='space-y-1'>
                            <Label className='text-muted-foreground text-[10px]'>
                              Sale Price *
                            </Label>
                            <Input
                              type='number'
                              step='any'
                              value={row.amount}
                              onChange={(e) =>
                                handlePriceChange(idx, 'amount', e.target.value)
                              }
                              className='h-8 rounded-lg font-mono text-xs font-semibold'
                            />
                          </div>

                          {/* Compare-at Price */}
                          <div className='space-y-1'>
                            <div className='flex items-center justify-between'>
                              <Label className='text-muted-foreground text-[10px]'>
                                Compare-at Price
                              </Label>
                              {(!editingVariant || formPrices.length > 1) && (
                                <Button
                                  type='button'
                                  variant='ghost'
                                  size='icon'
                                  onClick={() => handleRemovePriceRow(idx)}
                                  className='text-destructive h-6 w-6'
                                  aria-label={combinationsCopy('removePrice')}
                                >
                                  <Trash2 className='h-3 w-3' />
                                </Button>
                              )}
                            </div>
                            <Input
                              type='number'
                              step='any'
                              value={row.compareAtAmount}
                              onChange={(e) =>
                                handlePriceChange(
                                  idx,
                                  'compareAtAmount',
                                  e.target.value
                                )
                              }
                              placeholder='0.00'
                              className='h-8 rounded-lg font-mono text-xs'
                            />
                          </div>
                        </div>

                        {/* Quick Discount Buttons & Sale Indicator */}
                        <div className='border-border/40 flex items-center justify-between border-t pt-1 text-[10px]'>
                          <div className='flex items-center gap-1'>
                            <span className='text-muted-foreground'>
                              Discount:
                            </span>
                            {[10, 15, 20, 30].map((pct) => (
                              <button
                                key={pct}
                                type='button'
                                onClick={() =>
                                  handleApplyDiscountPercent(idx, pct)
                                }
                                className='bg-muted/60 hover:bg-muted text-muted-foreground hover:text-foreground rounded px-1.5 py-0.5 transition'
                              >
                                -{pct}%
                              </button>
                            ))}
                          </div>
                          {hasDiscount && (
                            <Badge
                              variant='outline'
                              className='border-rose-200 bg-rose-50 px-1.5 py-0 text-[9px] text-rose-700 dark:bg-rose-950/40 dark:text-rose-300'
                            >
                              {discountPct}% OFF (ON SALE)
                            </Badge>
                          )}
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>

              {/* Inventory and Policy */}
              <div className='grid grid-cols-2 gap-3'>
                <div className='space-y-1.5'>
                  <div className='flex items-center justify-between'>
                    <Label htmlFor='var-stock'>Stock on Hand</Label>
                    <div className='flex items-center gap-1'>
                      <button
                        type='button'
                        onClick={() =>
                          setFormStock(
                            Math.max(
                              0,
                              (parseInt(formStock, 10) || 0) - 1
                            ).toString()
                          )
                        }
                        className='bg-muted hover:bg-muted/80 h-5 w-5 rounded text-[10px] font-bold'
                        title='-1 pc'
                      >
                        -1
                      </button>
                      <button
                        type='button'
                        onClick={() =>
                          setFormStock(
                            ((parseInt(formStock, 10) || 0) + 1).toString()
                          )
                        }
                        className='bg-muted hover:bg-muted/80 h-5 w-5 rounded text-[10px] font-bold'
                        title='+1 pc'
                      >
                        +1
                      </button>
                      <button
                        type='button'
                        onClick={() =>
                          setFormStock(
                            ((parseInt(formStock, 10) || 0) + 10).toString()
                          )
                        }
                        className='bg-muted hover:bg-muted/80 h-5 rounded px-1 text-[10px] font-medium'
                        title='+10 pcs'
                      >
                        +10
                      </button>
                    </div>
                  </div>
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
                        Deny when out of stock (Deny)
                      </SelectItem>
                      <SelectItem value='continue' className='text-xs'>
                        Allow backorders (Continue)
                      </SelectItem>
                    </SelectContent>
                  </Select>
                </div>
              </div>
            </div>

            {error && (
              <p role='alert' className='text-destructive text-sm'>
                {error}
              </p>
            )}
            {axes.length > 0 && !editingVariant ? (
              <div className='grid gap-3'>
                {axes.map((axis) => (
                  <div key={axis.attributeId} className='space-y-1'>
                    <Label htmlFor={`sku-axis-${axis.attributeId}`}>
                      {axis.name}
                    </Label>
                    <Select
                      disabled={disabled || isBusy}
                      value={
                        formAxisValues.find(
                          (value) => value.attributeId === axis.attributeId
                        )?.optionId ?? ''
                      }
                      onValueChange={(optionId) =>
                        setFormAxisValues((current) => [
                          ...current.filter(
                            (value) => value.attributeId !== axis.attributeId
                          ),
                          { attributeId: axis.attributeId, optionId }
                        ])
                      }
                    >
                      <SelectTrigger id={`sku-axis-${axis.attributeId}`}>
                        <SelectValue placeholder='Choose an allowed value' />
                      </SelectTrigger>
                      <SelectContent>
                        {axis.allowedValues.map((value) => (
                          <SelectItem
                            key={value.optionId}
                            value={value.optionId}
                          >
                            {value.value}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                ))}
              </div>
            ) : null}
            <DialogFooter className='gap-2 pt-2 sm:gap-0'>
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
                    ? 'Save Variant'
                    : 'Create Variant'}
              </Button>
            </DialogFooter>
          </fieldset>
        </DialogContent>
      </Dialog>
    </Card>
  );
}
