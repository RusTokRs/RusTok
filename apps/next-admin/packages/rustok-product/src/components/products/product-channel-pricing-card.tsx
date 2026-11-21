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
import { Loader2, Tags } from 'lucide-react';
import { toast } from 'sonner';
import type {
  ActivePriceList,
  PricingAdjustmentPreview,
  ProductVariant
} from '../../api/types';
import { SearchableSelect } from '../shared/searchable-select';

const PRICE_LIST_EMPTY = '';

export interface ProductChannelPricingCardProps {
  variants: ProductVariant[];
  onFetchPriceLists?: (
    channelId?: string,
    channelSlug?: string
  ) => Promise<ActivePriceList[]>;
  onUpsertPrice?: (
    variantId: string,
    input: {
      currencyCode: string;
      amount: string;
      compareAtAmount?: string | null;
      priceListId?: string | null;
      channelId?: string | null;
      channelSlug?: string | null;
      minQuantity?: number | null;
      maxQuantity?: number | null;
    }
  ) => Promise<void>;
  onPreviewDiscount?: (
    variantId: string,
    input: { currencyCode: string; discountPercent: string; priceListId?: string | null }
  ) => Promise<PricingAdjustmentPreview>;
  onApplyDiscount?: (
    variantId: string,
    input: { currencyCode: string; discountPercent: string; priceListId?: string | null }
  ) => Promise<void>;
  disabled?: boolean;
}

/**
 * Channel-contextual pricing for one variant: price-list overrides and
 * percentage discounts through the Pricing owner. Base prices are owned by the
 * catalog variant editor and are deliberately not editable here.
 */
export function ProductChannelPricingCard({
  variants,
  onFetchPriceLists,
  onUpsertPrice,
  onPreviewDiscount,
  onApplyDiscount,
  disabled = false
}: ProductChannelPricingCardProps) {
  const [variantId, setVariantId] = React.useState('');
  const [priceLists, setPriceLists] = React.useState<ActivePriceList[] | null>(
    null
  );
  const [priceListId, setPriceListId] = React.useState(PRICE_LIST_EMPTY);
  const [currencyCode, setCurrencyCode] = React.useState('USD');
  const [amount, setAmount] = React.useState('');
  const [compareAtAmount, setCompareAtAmount] = React.useState('');
  const [minQuantity, setMinQuantity] = React.useState('');
  const [maxQuantity, setMaxQuantity] = React.useState('');
  const [discountPercent, setDiscountPercent] = React.useState('');
  const [preview, setPreview] = React.useState<PricingAdjustmentPreview | null>(
    null
  );
  const [pending, setPending] = React.useState<string | null>(null);

  const selectedVariant = React.useMemo(
    () => variants.find((variant) => variant.id === variantId) ?? null,
    [variants, variantId]
  );

  React.useEffect(() => {
    if (!variantId || priceLists !== null || !onFetchPriceLists) return;
    let cancelled = false;
    setPending('priceLists');
    onFetchPriceLists()
      .then((lists) => {
        if (!cancelled) setPriceLists(lists);
      })
      .catch((err) => {
        if (!cancelled) {
          setPriceLists([]);
          toast.error(
            err instanceof Error
              ? err.message
              : 'Failed to load active price lists.'
          );
        }
      })
      .finally(() => {
        if (!cancelled) setPending(null);
      });
    return () => {
      cancelled = true;
    };
  }, [variantId, priceLists, onFetchPriceLists]);

  React.useEffect(() => {
    setPriceListId(PRICE_LIST_EMPTY);
    setPreview(null);
    setAmount('');
    setCompareAtAmount('');
    setDiscountPercent('');
    const baseCurrency = selectedVariant?.prices?.[0]?.currencyCode;
    setCurrencyCode(baseCurrency ?? 'USD');
  }, [variantId, selectedVariant]);

  const selectedPriceList = React.useMemo(
    () => priceLists?.find((list) => list.id === priceListId) ?? null,
    [priceLists, priceListId]
  );

  const handleUpsert = async () => {
    if (!onUpsertPrice || !selectedVariant) return;
    if (!amount.trim()) {
      toast.error('Amount is required.');
      return;
    }
    if (Number.isNaN(Number(amount))) {
      toast.error('Amount must be a decimal number.');
      return;
    }
    setPending('upsert');
    try {
      await onUpsertPrice(selectedVariant.id, {
        currencyCode: currencyCode.trim().toUpperCase(),
        amount: amount.trim(),
        compareAtAmount: compareAtAmount.trim() || null,
        priceListId: priceListId || null,
        channelId: selectedPriceList?.channelId ?? null,
        channelSlug: selectedPriceList?.channelSlug ?? null,
        minQuantity: minQuantity.trim() ? Number(minQuantity.trim()) : null,
        maxQuantity: maxQuantity.trim() ? Number(maxQuantity.trim()) : null
      });
      toast.success('Variant price saved.');
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : 'Failed to save variant price.'
      );
    } finally {
      setPending(null);
    }
  };

  const handlePreviewDiscount = async () => {
    if (!onPreviewDiscount || !selectedVariant) return;
    const percent = discountPercent.trim();
    if (!percent) {
      toast.error('Discount percent is required.');
      return;
    }
    const parsed = Number(percent);
    if (Number.isNaN(parsed) || parsed <= 0 || parsed >= 100) {
      toast.error('Discount percent must be between 0 and 100 (exclusive).');
      return;
    }
    setPending('preview');
    try {
      const result = await onPreviewDiscount(selectedVariant.id, {
        currencyCode: currencyCode.trim().toUpperCase(),
        discountPercent: percent,
        priceListId: priceListId || null
      });
      setPreview(result);
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : 'Failed to preview discount.'
      );
    } finally {
      setPending(null);
    }
  };

  const handleApplyDiscount = async () => {
    if (!onApplyDiscount || !selectedVariant) return;
    const percent = discountPercent.trim();
    if (!percent) {
      toast.error('Discount percent is required.');
      return;
    }
    setPending('apply');
    try {
      await onApplyDiscount(selectedVariant.id, {
        currencyCode: currencyCode.trim().toUpperCase(),
        discountPercent: percent,
        priceListId: priceListId || null
      });
      toast.success('Variant discount applied.');
      setPreview(null);
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : 'Failed to apply discount.'
      );
    } finally {
      setPending(null);
    }
  };

  if (variants.length === 0) return null;

  return (
    <Card className='border-border rounded-2xl shadow-sm'>
      <CardHeader className='border-border/60 border-b pb-4'>
        <div className='flex items-center gap-2'>
          <Tags className='text-primary h-4 w-4' />
          <div>
            <CardTitle className='text-sm font-semibold'>
              Channel Pricing & Discounts
            </CardTitle>
            <CardDescription className='text-xs'>
              Price-list overrides and percentage discounts through the Pricing
              owner. Base prices are edited on the variant itself.
            </CardDescription>
          </div>
        </div>
      </CardHeader>
      <CardContent className='space-y-4 pt-5'>
        <div className='grid gap-3 md:grid-cols-2'>
          <div className='space-y-1.5'>
            <Label htmlFor='pricing-variant'>Variant</Label>
            <SearchableSelect
              id='pricing-variant'
              options={variants.map((variant) => ({
                value: variant.id,
                label: variant.sku || variant.title || variant.id
              }))}
              value={variantId}
              emptyValue={PRICE_LIST_EMPTY}
              emptyLabel='Select a variant...'
              searchPlaceholder='Search variants by SKU...'
              disabled={disabled}
              onChange={setVariantId}
            />
          </div>
          <div className='space-y-1.5'>
            <Label htmlFor='pricing-price-list'>Price list (optional)</Label>
            <SearchableSelect
              id='pricing-price-list'
              options={(priceLists ?? []).map((list) => ({
                value: list.id,
                label: list.channelSlug
                  ? `${list.name} (${list.channelSlug})`
                  : list.name
              }))}
              value={priceListId}
              emptyValue={PRICE_LIST_EMPTY}
              emptyLabel='Direct price (no list)'
              searchPlaceholder='Search price lists...'
              disabled={disabled || !variantId}
              onChange={setPriceListId}
            />
          </div>
        </div>

        {variantId && (
          <>
            <div className='grid gap-3 border-border/40 border-t pt-4 md:grid-cols-4'>
              <div className='space-y-1.5'>
                <Label htmlFor='pricing-currency'>Currency</Label>
                <Input
                  id='pricing-currency'
                  value={currencyCode}
                  onChange={(e) => setCurrencyCode(e.target.value)}
                  placeholder='USD'
                  disabled={disabled}
                  className='h-9 rounded-xl text-xs'
                />
              </div>
              <div className='space-y-1.5'>
                <Label htmlFor='pricing-amount'>Amount *</Label>
                <Input
                  id='pricing-amount'
                  value={amount}
                  onChange={(e) => setAmount(e.target.value)}
                  placeholder='129.99'
                  disabled={disabled}
                  className='h-9 rounded-xl text-xs'
                />
              </div>
              <div className='space-y-1.5'>
                <Label htmlFor='pricing-compare-at'>Compare at</Label>
                <Input
                  id='pricing-compare-at'
                  value={compareAtAmount}
                  onChange={(e) => setCompareAtAmount(e.target.value)}
                  placeholder='159.99'
                  disabled={disabled}
                  className='h-9 rounded-xl text-xs'
                />
              </div>
              <div className='grid grid-cols-2 gap-2'>
                <div className='space-y-1.5'>
                  <Label htmlFor='pricing-min-qty'>Min qty</Label>
                  <Input
                    id='pricing-min-qty'
                    value={minQuantity}
                    onChange={(e) => setMinQuantity(e.target.value)}
                    placeholder='1'
                    disabled={disabled}
                    className='h-9 rounded-xl text-xs'
                  />
                </div>
                <div className='space-y-1.5'>
                  <Label htmlFor='pricing-max-qty'>Max qty</Label>
                  <Input
                    id='pricing-max-qty'
                    value={maxQuantity}
                    onChange={(e) => setMaxQuantity(e.target.value)}
                    placeholder='10'
                    disabled={disabled}
                    className='h-9 rounded-xl text-xs'
                  />
                </div>
              </div>
            </div>
            <div className='flex justify-end'>
              <Button
                type='button'
                size='sm'
                onClick={handleUpsert}
                disabled={disabled || pending !== null || !onUpsertPrice}
                className='rounded-xl text-xs font-semibold'
              >
                {pending === 'upsert' ? (
                  <Loader2 className='mr-1.5 h-3.5 w-3.5 animate-spin' />
                ) : null}
                Save Channel Price
              </Button>
            </div>

            <div className='grid gap-3 border-border/40 border-t pt-4 md:grid-cols-4'>
              <div className='space-y-1.5'>
                <Label htmlFor='pricing-discount'>Discount %</Label>
                <Input
                  id='pricing-discount'
                  value={discountPercent}
                  onChange={(e) => setDiscountPercent(e.target.value)}
                  placeholder='15'
                  disabled={disabled}
                  className='h-9 rounded-xl text-xs'
                />
              </div>
              <div className='flex items-end gap-2 md:col-span-3'>
                <Button
                  type='button'
                  size='sm'
                  variant='outline'
                  onClick={handlePreviewDiscount}
                  disabled={disabled || pending !== null || !onPreviewDiscount}
                  className='rounded-xl text-xs'
                >
                  {pending === 'preview' ? (
                    <Loader2 className='mr-1.5 h-3.5 w-3.5 animate-spin' />
                  ) : null}
                  Preview
                </Button>
                <Button
                  type='button'
                  size='sm'
                  onClick={handleApplyDiscount}
                  disabled={
                    disabled ||
                    pending !== null ||
                    !onApplyDiscount ||
                    !discountPercent.trim()
                  }
                  className='rounded-xl text-xs font-semibold'
                >
                  {pending === 'apply' ? (
                    <Loader2 className='mr-1.5 h-3.5 w-3.5 animate-spin' />
                  ) : null}
                  Apply Discount
                </Button>
                {preview && (
                  <p className='text-muted-foreground text-xs'>
                    {preview.currentAmount} {preview.currencyCode} →{' '}
                    <span className='text-foreground font-medium'>
                      {preview.adjustedAmount} {preview.currencyCode}
                    </span>{' '}
                    (−{preview.adjustmentPercent}%)
                  </p>
                )}
              </div>
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}
