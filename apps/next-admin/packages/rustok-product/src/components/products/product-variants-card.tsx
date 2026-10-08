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
import type {
  ProductVariant,
  ProductVariantPrice,
  ActivePriceList
} from '../../api/types';

interface PriceRowDraft {
  currencyCode: string;
  amount: string;
  compareAtAmount: string;
  priceListId?: string;
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
  onAddVariant?: (variant: Omit<ProductVariant, 'id'>) => Promise<void>;
  onUpdateVariant?: (
    id: string,
    variant: Partial<ProductVariant>
  ) => Promise<void>;
  onDeleteVariant?: (id: string) => Promise<void>;
  isNew: boolean;
  activePriceLists?: ActivePriceList[];
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
  activePriceLists = [],
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
  const [formPrices, setFormPrices] = React.useState<PriceRowDraft[]>([
    { currencyCode: 'USD', amount: '0.00', compareAtAmount: '' }
  ]);
  const [formStock, setFormStock] = React.useState('0');
  const [formPolicy, setFormPolicy] = React.useState('deny');

  const openAddDialog = () => {
    setEditingVariant(null);
    setFormSku('');
    setFormBarcode('');
    setFormPrices([
      { currencyCode: 'USD', amount: '0.00', compareAtAmount: '' }
    ]);
    setFormStock('0');
    setFormPolicy('deny');
    setDialogOpen(true);
  };

  const openEditDialog = (variant: ProductVariant) => {
    setEditingVariant(variant);
    setFormSku(variant.sku || '');
    setFormBarcode(variant.barcode || '');

    if (variant.prices && variant.prices.length > 0) {
      setFormPrices(
        variant.prices.map((p) => ({
          currencyCode: p.currencyCode,
          amount: (p.amount / 100).toFixed(2),
          compareAtAmount: p.compareAtAmount
            ? (p.compareAtAmount / 100).toFixed(2)
            : '',
          priceListId: p.priceListId || undefined
        }))
      );
    } else {
      setFormPrices([
        { currencyCode: 'USD', amount: '0.00', compareAtAmount: '' }
      ]);
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
    setFormPrices((prev) => {
      const next = [...prev];
      const row = next[index];
      const base = parseFloat(row.compareAtAmount || row.amount);
      if (Number.isFinite(base) && base > 0) {
        const compareAt = row.compareAtAmount ? base : parseFloat(row.amount);
        const discounted = compareAt * (1 - percent / 100);
        next[index] = {
          ...row,
          compareAtAmount: compareAt.toFixed(2),
          amount: Math.max(0, discounted).toFixed(2)
        };
      }
      return next;
    });
  };

  const handleAddPriceRow = () => {
    const existingCurrencies = new Set(formPrices.map((p) => p.currencyCode));
    const nextCur =
      SUPPORTED_CURRENCIES.find((c) => !existingCurrencies.has(c.code))?.code ||
      'EUR';
    setFormPrices((prev) => [
      ...prev,
      { currencyCode: nextCur, amount: '0.00', compareAtAmount: '' }
    ]);
  };

  const handleRemovePriceRow = (index: number) => {
    if (formPrices.length <= 1) return;
    setFormPrices((prev) => prev.filter((_, i) => i !== index));
  };

  const handleSaveDialog = async () => {
    const qty = parseInt(formStock, 10) || 0;

    const mappedPrices: ProductVariantPrice[] = formPrices.map((p) => {
      const priceRaw = parseFloat(p.amount);
      const amount =
        Number.isFinite(priceRaw) && priceRaw > 0
          ? Math.round(priceRaw * 100)
          : 0;
      const compareRaw = parseFloat(p.compareAtAmount);
      const compareAtAmount =
        Number.isFinite(compareRaw) && compareRaw > 0
          ? Math.round(compareRaw * 100)
          : null;
      const onSale = Boolean(compareAtAmount && compareAtAmount > amount);
      const discountPercent = onSale
        ? Math.round(
            ((compareAtAmount! - amount) / compareAtAmount!) * 100
          ).toString()
        : null;

      return {
        currencyCode: p.currencyCode,
        amount,
        compareAtAmount,
        discountPercent,
        onSale,
        priceListId: p.priceListId || null
      };
    });

    setIsBusy(true);
    try {
      if (editingVariant && onUpdateVariant) {
        await onUpdateVariant(editingVariant.id, {
          sku: formSku || null,
          barcode: formBarcode || null,
          inventoryQuantity: qty,
          inventoryPolicy: formPolicy,
          prices: mappedPrices
        });
      } else if (onAddVariant) {
        await onAddVariant({
          sku: formSku || null,
          barcode: formBarcode || null,
          title: formSku ? `Variant ${formSku}` : null,
          inventoryQuantity: qty,
          inventoryPolicy: formPolicy,
          inStock: qty > 0,
          prices: mappedPrices
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
                Цены и варианты (Pricing & Variants Matrix)
              </CardTitle>
              <CardDescription className='text-xs'>
                SKU, штрихкоды, мультивалютная сетка цен, скидки и остатки
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
              Добавить вариант
            </Button>
          )}
        </div>
      </CardHeader>
      <CardContent className='pt-4'>
        {isNew ? (
          <div className='grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4'>
            <div className='space-y-1.5'>
              <Label htmlFor='new-sku' className='text-xs font-medium'>
                SKU (Артикул)
              </Label>
              <Input
                id='new-sku'
                value={newVariantDraft?.sku || ''}
                onChange={(e) =>
                  onNewVariantDraftChange?.({
                    ...(newVariantDraft || {
                      sku: '',
                      barcode: '',
                      priceAmount: 0,
                      currencyCode: 'USD',
                      inventoryQuantity: 0,
                      inventoryPolicy: 'deny'
                    }),
                    sku: e.target.value
                  })
                }
                placeholder='e.g. PROD-001'
                className='h-9 rounded-xl font-mono text-xs'
                disabled={disabled}
              />
            </div>
            <div className='space-y-1.5'>
              <Label htmlFor='new-barcode' className='text-xs font-medium'>
                Штрихкод (Barcode / EAN)
              </Label>
              <Input
                id='new-barcode'
                value={newVariantDraft?.barcode || ''}
                onChange={(e) =>
                  onNewVariantDraftChange?.({
                    ...(newVariantDraft || {
                      sku: '',
                      barcode: '',
                      priceAmount: 0,
                      currencyCode: 'USD',
                      inventoryQuantity: 0,
                      inventoryPolicy: 'deny'
                    }),
                    barcode: e.target.value
                  })
                }
                placeholder='e.g. 793573192842'
                className='h-9 rounded-xl font-mono text-xs'
                disabled={disabled}
              />
            </div>
            <div className='space-y-1.5'>
              <Label htmlFor='new-price' className='text-xs font-medium'>
                Цена по умолчанию ({newVariantDraft?.currencyCode || 'USD'}) *
              </Label>
              <div className='relative'>
                <Input
                  id='new-price'
                  type='number'
                  step='0.01'
                  value={
                    newVariantDraft?.priceAmount
                      ? (newVariantDraft.priceAmount / 100).toFixed(2)
                      : '0.00'
                  }
                  onChange={(e) => {
                    const val = parseFloat(e.target.value);
                    onNewVariantDraftChange?.({
                      ...(newVariantDraft || {
                        sku: '',
                        barcode: '',
                        priceAmount: 0,
                        currencyCode: 'USD',
                        inventoryQuantity: 0,
                        inventoryPolicy: 'deny'
                      }),
                      priceAmount:
                        Number.isFinite(val) && val > 0
                          ? Math.round(val * 100)
                          : 0
                    });
                  }}
                  className='h-9 rounded-xl font-mono text-xs'
                  disabled={disabled}
                />
              </div>
            </div>
            <div className='space-y-1.5'>
              <Label htmlFor='new-stock' className='text-xs font-medium'>
                Начальный остаток
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
            Нет вариантов для этого товара. Нажмите «Добавить вариант».
          </div>
        ) : (
          <div className='border-border overflow-hidden rounded-xl border'>
            <Table>
              <TableHeader className='bg-muted/50'>
                <TableRow className='text-[11px]'>
                  <TableHead>SKU / Штрихкод</TableHead>
                  <TableHead>Основная цена</TableHead>
                  <TableHead>Валютная сетка</TableHead>
                  <TableHead>Остаток</TableHead>
                  <TableHead>Политика</TableHead>
                  <TableHead className='text-right'>Действия</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody className='text-xs'>
                {variants.map((v) => {
                  const primaryPrice = v.prices[0];
                  const additionalPrices = v.prices.slice(1);
                  const isOnSale = Boolean(
                    primaryPrice?.compareAtAmount &&
                    primaryPrice.compareAtAmount > primaryPrice.amount
                  );
                  const discountPct = isOnSale
                    ? Math.round(
                        ((primaryPrice!.compareAtAmount! -
                          primaryPrice!.amount) /
                          primaryPrice!.compareAtAmount!) *
                          100
                      )
                    : null;

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
                            Одна валюта
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
                            {v.inventoryQuantity} шт.
                            {v.inventoryQuantity <= 0
                              ? ' (0)'
                              : v.inventoryQuantity <= 5
                                ? ' (Мало)'
                                : ''}
                          </Badge>
                          {onUpdateVariant && (
                            <div className='flex items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100'>
                              <button
                                type='button'
                                onClick={() =>
                                  onUpdateVariant(v.id, {
                                    inventoryQuantity: Math.max(
                                      0,
                                      v.inventoryQuantity - 1
                                    )
                                  })
                                }
                                disabled={disabled || v.inventoryQuantity <= 0}
                                className='bg-muted hover:bg-muted/80 text-muted-foreground hover:text-foreground flex h-5 w-5 items-center justify-center rounded text-[11px] font-bold'
                                title='Уменьшить остаток на 1'
                              >
                                -
                              </button>
                              <button
                                type='button'
                                onClick={() =>
                                  onUpdateVariant(v.id, {
                                    inventoryQuantity: v.inventoryQuantity + 1
                                  })
                                }
                                disabled={disabled}
                                className='bg-muted hover:bg-muted/80 text-muted-foreground hover:text-foreground flex h-5 w-5 items-center justify-center rounded text-[11px] font-bold'
                                title='Увеличить остаток на 1'
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
                              disabled={disabled}
                              title='Редактировать вариант и цены'
                            >
                              <Edit2 className='h-3.5 w-3.5' />
                            </Button>
                          )}
                          {onDeleteVariant && variants.length > 1 && (
                            <Button
                              type='button'
                              variant='ghost'
                              size='icon'
                              onClick={() => onDeleteVariant(v.id)}
                              className='h-7 w-7 rounded-lg text-rose-500 hover:bg-rose-50 hover:text-rose-700 dark:hover:bg-rose-950/40'
                              disabled={disabled}
                              title='Удалить вариант'
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
      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className='max-h-[90vh] overflow-y-auto rounded-2xl sm:max-w-lg'>
          <DialogHeader>
            <DialogTitle className='text-sm font-semibold'>
              {editingVariant
                ? 'Редактировать вариант'
                : 'Новый вариант товара'}
            </DialogTitle>
            <DialogDescription className='text-xs'>
              Настройте SKU, штрихкод, мультивалютную сетку цен и правила
              скидок.
            </DialogDescription>
          </DialogHeader>

          <div className='space-y-4 py-2 text-xs'>
            {/* SKU and Barcode */}
            <div className='grid grid-cols-2 gap-3'>
              <div className='space-y-1.5'>
                <Label htmlFor='var-sku'>SKU (Артикул)</Label>
                <Input
                  id='var-sku'
                  value={formSku}
                  onChange={(e) => setFormSku(e.target.value)}
                  placeholder='e.g. MOUSE-BLK'
                  className='h-9 rounded-xl font-mono text-xs'
                />
              </div>
              <div className='space-y-1.5'>
                <Label htmlFor='var-barcode'>Штрихкод (Barcode / EAN)</Label>
                <Input
                  id='var-barcode'
                  value={formBarcode}
                  onChange={(e) => setFormBarcode(e.target.value)}
                  placeholder='e.g. 123456789012'
                  className='h-9 rounded-xl font-mono text-xs'
                />
              </div>
            </div>

            {/* Pricing Section - Multi-Currency Grid */}
            <div className='border-border bg-muted/10 space-y-3 rounded-xl border p-3.5'>
              <div className='flex items-center justify-between'>
                <div className='text-foreground flex items-center gap-1.5 text-xs font-semibold'>
                  <Coins className='text-primary h-3.5 w-3.5' />
                  <span>Валютная сетка и цены</span>
                </div>
                <Button
                  type='button'
                  variant='outline'
                  size='sm'
                  onClick={handleAddPriceRow}
                  className='h-7 gap-1 rounded-lg px-2 text-[11px]'
                >
                  <Plus className='h-3 w-3' />
                  Добавить валюту
                </Button>
              </div>

              {/* Price Rows */}
              <div className='space-y-2.5'>
                {formPrices.map((row, idx) => {
                  const amt = parseFloat(row.amount) || 0;
                  const comp = parseFloat(row.compareAtAmount) || 0;
                  const hasDiscount = comp > amt && amt > 0;
                  const discountPct = hasDiscount
                    ? Math.round(((comp - amt) / comp) * 100)
                    : 0;

                  return (
                    <div
                      key={idx}
                      className='border-border/80 bg-background space-y-2 rounded-xl border p-2.5'
                    >
                      <div className='grid grid-cols-3 items-end gap-2.5'>
                        {/* Currency */}
                        <div className='space-y-1'>
                          <Label className='text-muted-foreground text-[10px]'>
                            Валюта
                          </Label>
                          <Select
                            value={row.currencyCode}
                            onValueChange={(val) =>
                              handlePriceChange(idx, 'currencyCode', val)
                            }
                          >
                            <SelectTrigger className='h-8 rounded-lg font-mono text-xs'>
                              <SelectValue />
                            </SelectTrigger>
                            <SelectContent>
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
                            Цена продажи *
                          </Label>
                          <Input
                            type='number'
                            step='0.01'
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
                              Старая цена
                            </Label>
                            {formPrices.length > 1 && (
                              <button
                                type='button'
                                onClick={() => handleRemovePriceRow(idx)}
                                className='text-[10px] text-rose-500 hover:text-rose-700'
                                title='Удалить цену в этой валюте'
                              >
                                ✕
                              </button>
                            )}
                          </div>
                          <Input
                            type='number'
                            step='0.01'
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
                          <span className='text-muted-foreground'>Скидка:</span>
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
                            Скидка {discountPct}% (ON SALE)
                          </Badge>
                        )}
                      </div>

                      {/* Optional Price List Association */}
                      {activePriceLists.length > 0 && (
                        <div className='pt-1'>
                          <Select
                            value={row.priceListId || 'base'}
                            onValueChange={(val) =>
                              handlePriceChange(
                                idx,
                                'priceListId',
                                val === 'base' ? '' : val
                              )
                            }
                          >
                            <SelectTrigger className='h-7 rounded-lg text-[10px]'>
                              <SelectValue placeholder='Прайс-лист' />
                            </SelectTrigger>
                            <SelectContent>
                              <SelectItem value='base' className='text-xs'>
                                Базовая цена (Все покупатели)
                              </SelectItem>
                              {activePriceLists.map((pl) => (
                                <SelectItem
                                  key={pl.id}
                                  value={pl.id}
                                  className='text-xs'
                                >
                                  {pl.name} ({pl.listType})
                                </SelectItem>
                              ))}
                            </SelectContent>
                          </Select>
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>

            {/* Inventory and Policy */}
            <div className='grid grid-cols-2 gap-3'>
              <div className='space-y-1.5'>
                <div className='flex items-center justify-between'>
                  <Label htmlFor='var-stock'>Остаток на складе</Label>
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
                      title='-1 шт.'
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
                      title='+1 шт.'
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
                      title='+10 шт.'
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
                <Label htmlFor='var-policy'>Политика списания</Label>
                <Select value={formPolicy} onValueChange={setFormPolicy}>
                  <SelectTrigger
                    id='var-policy'
                    className='h-9 rounded-xl text-xs'
                  >
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value='deny' className='text-xs'>
                      Блокировать при 0 остатке (Deny)
                    </SelectItem>
                    <SelectItem value='continue' className='text-xs'>
                      Разрешить предзаказ (Backorder / Continue)
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>
          </div>

          <DialogFooter className='gap-2 pt-2 sm:gap-0'>
            <Button
              type='button'
              variant='outline'
              onClick={() => setDialogOpen(false)}
              className='h-9 rounded-xl text-xs'
            >
              Отмена
            </Button>
            <Button
              type='button'
              onClick={handleSaveDialog}
              disabled={isBusy}
              className='h-9 rounded-xl text-xs font-semibold'
            >
              {isBusy
                ? 'Сохранение...'
                : editingVariant
                  ? 'Сохранить вариант'
                  : 'Создать вариант'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Card>
  );
}
