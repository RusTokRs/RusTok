'use client';

import React from 'react';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import { Badge } from '@/shared/ui/shadcn/badge';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from '@/widgets/data-table';
import { PageContainer } from '@/widgets/app-shell';
import { previewCartPromotion, applyCartPromotion, GqlOpts } from '../api';
import type { CartPromotionPreview, CartSnapshot } from '../types';

export function CartPromotionsTemplate({ opts }: { opts: GqlOpts }) {
  const [cartId, setCartId] = React.useState('');
  const [sourceId, setSourceId] = React.useState('');
  const [kind, setKind] = React.useState('PERCENT');
  const [scope, setScope] = React.useState('CART');
  const [lineItemId, setLineItemId] = React.useState('');
  const [discountPercent, setDiscountPercent] = React.useState('');
  const [amount, setAmount] = React.useState('');
  const [metadataStr, setMetadataStr] = React.useState('{}');

  const [previewResult, setPreviewResult] =
    React.useState<CartPromotionPreview | null>(null);
  const [cartSnapshot, setCartSnapshot] = React.useState<CartSnapshot | null>(
    null
  );

  const [loading, setLoading] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [feedback, setFeedback] = React.useState<string | null>(null);

  const getPromotionInput = () => {
    return {
      sourceId: sourceId.trim(),
      kind,
      scope,
      lineItemId: lineItemId.trim() || null,
      discountPercent: discountPercent ? discountPercent : null,
      amount: amount ? amount : null,
      metadata: metadataStr || null
    };
  };

  const handlePreview = async (e: React.MouseEvent) => {
    e.preventDefault();
    if (!cartId.trim() || !sourceId.trim()) {
      setError('Cart ID and Source ID are required to run preview.');
      return;
    }
    setError(null);
    setFeedback(null);
    setLoading(true);
    try {
      const preview = await previewCartPromotion(
        opts,
        cartId.trim(),
        getPromotionInput()
      );
      setPreviewResult(preview);
      setFeedback('Promotion preview loaded successfully.');
    } catch (err) {
      setError(
        err instanceof Error ? err.message : 'Failed to preview cart promotion.'
      );
    } finally {
      setLoading(false);
    }
  };

  const handleApply = async (e: React.MouseEvent) => {
    e.preventDefault();
    if (!cartId.trim() || !sourceId.trim()) {
      setError('Cart ID and Source ID are required to apply promotion.');
      return;
    }
    setError(null);
    setFeedback(null);
    setLoading(true);
    try {
      const snapshot = await applyCartPromotion(
        opts,
        cartId.trim(),
        getPromotionInput()
      );
      setCartSnapshot(snapshot);
      setFeedback('Promotion applied to cart successfully.');
    } catch (err) {
      setError(
        err instanceof Error ? err.message : 'Failed to apply cart promotion.'
      );
    } finally {
      setLoading(false);
    }
  };

  return (
    <PageContainer
      pageTitle='Cart Promotions'
      pageDescription='Test, preview, and apply targeted promotions to shopping carts.'
    >
      <div className='space-y-6'>
        {feedback && (
          <div className='rounded-lg border border-emerald-300 bg-emerald-50 px-4 py-3 text-sm text-emerald-700'>
            {feedback}
          </div>
        )}
        {error && (
          <div className='border-destructive/30 bg-destructive/10 text-destructive rounded-lg border px-4 py-3 text-sm'>
            {error}
          </div>
        )}

        <div className='grid gap-6 lg:grid-cols-2'>
          {/* Form Card */}
          <Card>
            <CardHeader>
              <CardTitle className='text-base'>Promotion Settings</CardTitle>
              <CardDescription>
                Specify the target cart and promo rules.
              </CardDescription>
            </CardHeader>
            <CardContent className='space-y-4'>
              <div className='space-y-2'>
                <label className='text-xs font-semibold'>Cart ID (UUID)</label>
                <Input
                  required
                  placeholder='e.g. 123e4567-e89b-12d3-a456-426614174000'
                  value={cartId}
                  onChange={(e) => setCartId(e.target.value)}
                />
              </div>

              <div className='space-y-2'>
                <label className='text-xs font-semibold'>
                  Source ID (Promo / Campaign ID)
                </label>
                <Input
                  required
                  placeholder='e.g. promo-code-summer'
                  value={sourceId}
                  onChange={(e) => setSourceId(e.target.value)}
                />
              </div>

              <div className='grid grid-cols-2 gap-4'>
                <div className='space-y-2'>
                  <label className='text-xs font-semibold'>Kind</label>
                  <select
                    className='border-input bg-background w-full rounded-md border px-3 py-2 text-sm shadow-sm'
                    value={kind}
                    onChange={(e) => setKind(e.target.value)}
                  >
                    <option value='PERCENT'>PERCENT</option>
                    <option value='AMOUNT'>AMOUNT</option>
                  </select>
                </div>
                <div className='space-y-2'>
                  <label className='text-xs font-semibold'>Scope</label>
                  <select
                    className='border-input bg-background w-full rounded-md border px-3 py-2 text-sm shadow-sm'
                    value={scope}
                    onChange={(e) => setScope(e.target.value)}
                  >
                    <option value='CART'>CART</option>
                    <option value='LINE_ITEM'>LINE ITEM</option>
                  </select>
                </div>
              </div>

              <div className='grid grid-cols-2 gap-4'>
                <div className='space-y-2'>
                  <label className='text-xs font-semibold'>
                    Discount Percent
                  </label>
                  <Input
                    placeholder='e.g. 15.00'
                    value={discountPercent}
                    onChange={(e) => setDiscountPercent(e.target.value)}
                    disabled={kind !== 'PERCENT'}
                  />
                </div>
                <div className='space-y-2'>
                  <label className='text-xs font-semibold'>
                    Discount Amount
                  </label>
                  <Input
                    placeholder='e.g. 10.00'
                    value={amount}
                    onChange={(e) => setAmount(e.target.value)}
                    disabled={kind !== 'AMOUNT'}
                  />
                </div>
              </div>

              {scope === 'LINE_ITEM' && (
                <div className='space-y-2'>
                  <label className='text-xs font-semibold'>
                    Line Item ID (UUID)
                  </label>
                  <Input
                    placeholder='e.g. 987f6543-e21b-34c5-d678-901112131415'
                    value={lineItemId}
                    onChange={(e) => setLineItemId(e.target.value)}
                  />
                </div>
              )}

              <div className='space-y-2'>
                <label className='text-xs font-semibold'>Metadata JSON</label>
                <Input
                  placeholder='{}'
                  value={metadataStr}
                  onChange={(e) => setMetadataStr(e.target.value)}
                />
              </div>

              <div className='flex gap-3 pt-2'>
                <Button
                  type='button'
                  variant='outline'
                  onClick={handlePreview}
                  disabled={loading}
                >
                  Preview Adjustment
                </Button>
                <Button type='button' onClick={handleApply} disabled={loading}>
                  Apply Promotion
                </Button>
              </div>
            </CardContent>
          </Card>

          {/* Results Card */}
          <div className='space-y-6'>
            {/* Preview Results */}
            {previewResult && (
              <Card className='border-sky-200 bg-sky-50/50'>
                <CardHeader>
                  <CardTitle className='text-base text-sky-900'>
                    Promotion Preview
                  </CardTitle>
                  <CardDescription>
                    Simulated values calculated on the server.
                  </CardDescription>
                </CardHeader>
                <CardContent className='space-y-3'>
                  <div className='grid grid-cols-2 gap-2 text-sm'>
                    <span className='text-muted-foreground font-medium'>
                      Kind:
                    </span>
                    <span>{previewResult.kind}</span>
                    <span className='text-muted-foreground font-medium'>
                      Scope:
                    </span>
                    <span>{previewResult.scope}</span>
                    <span className='text-muted-foreground font-medium'>
                      Original Base:
                    </span>
                    <span>
                      {previewResult.baseAmount} {previewResult.currencyCode}
                    </span>
                    <span className='text-muted-foreground font-medium'>
                      Discount Amount:
                    </span>
                    <span className='font-bold text-emerald-600'>
                      -{previewResult.adjustmentAmount}{' '}
                      {previewResult.currencyCode}
                    </span>
                    <span className='text-muted-foreground font-medium'>
                      Adjusted Total:
                    </span>
                    <span className='font-bold'>
                      {previewResult.adjustedAmount}{' '}
                      {previewResult.currencyCode}
                    </span>
                  </div>
                </CardContent>
              </Card>
            )}

            {/* Cart Snapshot Results */}
            {cartSnapshot && (
              <Card className='border-emerald-200 bg-emerald-50/30'>
                <CardHeader>
                  <CardTitle className='text-base text-emerald-950'>
                    Cart Snapshot
                  </CardTitle>
                  <CardDescription>
                    Updated live state of the cart after applying.
                  </CardDescription>
                </CardHeader>
                <CardContent className='space-y-4'>
                  <div className='grid grid-cols-2 gap-2 border-b pb-3 text-sm'>
                    <span className='text-muted-foreground font-medium'>
                      Cart ID:
                    </span>
                    <span className='font-mono text-xs'>{cartSnapshot.id}</span>
                    <span className='text-muted-foreground font-medium'>
                      Shipping Total:
                    </span>
                    <span>
                      {cartSnapshot.shippingTotal} {cartSnapshot.currencyCode}
                    </span>
                    <span className='text-muted-foreground font-medium'>
                      Adjustment Total:
                    </span>
                    <span className='font-semibold text-emerald-700'>
                      -{cartSnapshot.adjustmentTotal}{' '}
                      {cartSnapshot.currencyCode}
                    </span>
                    <span className='text-muted-foreground font-medium'>
                      Final Total:
                    </span>
                    <span className='text-base font-bold'>
                      {cartSnapshot.totalAmount} {cartSnapshot.currencyCode}
                    </span>
                  </div>

                  <div>
                    <h4 className='text-muted-foreground mb-2 text-xs font-bold uppercase'>
                      Applied Adjustments
                    </h4>
                    {cartSnapshot.adjustments.length === 0 ? (
                      <p className='text-muted-foreground text-xs'>
                        No adjustments active on this cart.
                      </p>
                    ) : (
                      <div className='bg-background rounded border text-xs'>
                        <Table>
                          <TableHeader>
                            <TableRow>
                              <TableHead>Source</TableHead>
                              <TableHead>Scope</TableHead>
                              <TableHead className='text-right'>
                                Amount
                              </TableHead>
                            </TableRow>
                          </TableHeader>
                          <TableBody>
                            {cartSnapshot.adjustments.map((adj) => (
                              <TableRow key={adj.id}>
                                <TableCell className='font-mono font-medium'>
                                  {adj.source || 'default'}
                                </TableCell>
                                <TableCell>{adj.scope}</TableCell>
                                <TableCell className='text-right font-medium text-emerald-600'>
                                  -{adj.amount}
                                </TableCell>
                              </TableRow>
                            ))}
                          </TableBody>
                        </Table>
                      </div>
                    )}
                  </div>
                </CardContent>
              </Card>
            )}
          </div>
        </div>
      </div>
    </PageContainer>
  );
}
