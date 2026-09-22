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
import { PageContainer } from '@/widgets/app-shell';
import { createOrderReturnDecision, GqlOpts } from '../api';
import type { ReturnDecisionResponse } from '../types';

export function ReturnDecisionsTemplate({ opts }: { opts: GqlOpts }) {
  const [orderId, setOrderId] = React.useState('');
  const [action, setAction] = React.useState('return_only');
  const [lineItemId, setLineItemId] = React.useState('');
  const [quantity, setQuantity] = React.useState('1');
  const [reason, setReason] = React.useState('');
  const [note, setNote] = React.useState('');
  const [paymentCollectionId, setPaymentCollectionId] = React.useState('');
  const [refundAmount, setRefundAmount] = React.useState('');
  const [resolutionDescription, setResolutionDescription] = React.useState('');
  const [resolutionPreview, setResolutionPreview] =
    React.useState('{"items":[]}');
  const [metadataStr, setMetadataStr] = React.useState('{}');
  const [result, setResult] = React.useState<ReturnDecisionResponse | null>(
    null
  );
  const [loading, setLoading] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [feedback, setFeedback] = React.useState<string | null>(null);

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    setError(null);
    setFeedback(null);
    setLoading(true);

    try {
      const itemQuantity = Number.parseInt(quantity, 10);
      if (
        !orderId.trim() ||
        !lineItemId.trim() ||
        !Number.isFinite(itemQuantity) ||
        itemQuantity <= 0
      ) {
        throw new Error(
          'Order ID, line item ID, and a positive quantity are required.'
        );
      }

      const response = await createOrderReturnDecision(opts, orderId.trim(), {
        returnRequest: {
          reason: reason.trim() || null,
          note: note.trim() || null,
          metadata: metadataStr.trim() || null,
          items: [
            {
              lineItemId: lineItemId.trim(),
              quantity: itemQuantity,
              reason: reason.trim() || null,
              note: note.trim() || null,
              metadata: null
            }
          ]
        },
        decision: {
          action,
          metadata: metadataStr.trim() || null,
          refund:
            action === 'refund'
              ? {
                  paymentCollectionId: paymentCollectionId.trim() || null,
                  amount: refundAmount.trim() || null,
                  reason: reason.trim() || null,
                  metadata: null
                }
              : null,
          exchange:
            action === 'exchange'
              ? {
                  description: resolutionDescription.trim() || null,
                  preview: resolutionPreview.trim() || null,
                  metadata: null
                }
              : null,
          claim:
            action === 'claim'
              ? {
                  description: resolutionDescription.trim() || null,
                  preview: resolutionPreview.trim() || null,
                  metadata: null
                }
              : null
        }
      });
      setResult(response);
      setFeedback(`Return decision created with action: ${response.action}`);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : 'Failed to create return decision.'
      );
    } finally {
      setLoading(false);
    }
  };

  return (
    <PageContainer
      pageTitle='Return Decisions'
      pageDescription='Create return-only, refund, exchange, or claim decisions through the commerce GraphQL transport.'
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

        <div className='grid gap-6 lg:grid-cols-[minmax(0,1.2fr)_minmax(320px,0.8fr)]'>
          <Card>
            <CardHeader>
              <CardTitle className='text-base'>Decision request</CardTitle>
              <CardDescription>
                This template keeps post-order orchestration in the module
                GraphQL adapter instead of the host route.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <form onSubmit={handleSubmit} className='space-y-4'>
                <div className='grid gap-4 md:grid-cols-2'>
                  <div className='space-y-2'>
                    <label className='text-xs font-semibold'>Order ID</label>
                    <Input
                      value={orderId}
                      onChange={(event) => setOrderId(event.target.value)}
                      placeholder='Order UUID'
                    />
                  </div>
                  <div className='space-y-2'>
                    <label className='text-xs font-semibold'>Action</label>
                    <select
                      className='border-input bg-background w-full rounded-md border px-3 py-2 text-sm shadow-sm'
                      value={action}
                      onChange={(event) => setAction(event.target.value)}
                    >
                      <option value='return_only'>Return only</option>
                      <option value='refund'>Refund</option>
                      <option value='exchange'>Exchange</option>
                      <option value='claim'>Claim</option>
                    </select>
                  </div>
                </div>

                <div className='grid gap-4 md:grid-cols-[minmax(0,1fr)_120px]'>
                  <div className='space-y-2'>
                    <label className='text-xs font-semibold'>
                      Line item ID
                    </label>
                    <Input
                      value={lineItemId}
                      onChange={(event) => setLineItemId(event.target.value)}
                      placeholder='Order line item UUID'
                    />
                  </div>
                  <div className='space-y-2'>
                    <label className='text-xs font-semibold'>Quantity</label>
                    <Input
                      value={quantity}
                      onChange={(event) => setQuantity(event.target.value)}
                      inputMode='numeric'
                    />
                  </div>
                </div>

                <div className='grid gap-4 md:grid-cols-2'>
                  <div className='space-y-2'>
                    <label className='text-xs font-semibold'>Reason</label>
                    <Input
                      value={reason}
                      onChange={(event) => setReason(event.target.value)}
                      placeholder='Customer returned item'
                    />
                  </div>
                  <div className='space-y-2'>
                    <label className='text-xs font-semibold'>Note</label>
                    <Input
                      value={note}
                      onChange={(event) => setNote(event.target.value)}
                      placeholder='Operator note'
                    />
                  </div>
                </div>

                {action === 'refund' && (
                  <div className='bg-background grid gap-4 rounded-lg border p-3 md:grid-cols-2'>
                    <div className='space-y-2'>
                      <label className='text-xs font-semibold'>
                        Payment collection ID
                      </label>
                      <Input
                        value={paymentCollectionId}
                        onChange={(event) =>
                          setPaymentCollectionId(event.target.value)
                        }
                        placeholder='Optional UUID'
                      />
                    </div>
                    <div className='space-y-2'>
                      <label className='text-xs font-semibold'>
                        Refund amount
                      </label>
                      <Input
                        value={refundAmount}
                        onChange={(event) =>
                          setRefundAmount(event.target.value)
                        }
                        placeholder='Optional amount'
                      />
                    </div>
                  </div>
                )}

                {(action === 'exchange' || action === 'claim') && (
                  <div className='bg-background space-y-3 rounded-lg border p-3'>
                    <div className='space-y-2'>
                      <label className='text-xs font-semibold'>
                        Resolution description
                      </label>
                      <Input
                        value={resolutionDescription}
                        onChange={(event) =>
                          setResolutionDescription(event.target.value)
                        }
                        placeholder={`${action} description`}
                      />
                    </div>
                    <div className='space-y-2'>
                      <label className='text-xs font-semibold'>
                        Resolution preview JSON
                      </label>
                      <Input
                        value={resolutionPreview}
                        onChange={(event) =>
                          setResolutionPreview(event.target.value)
                        }
                        placeholder='{}'
                      />
                    </div>
                  </div>
                )}

                <div className='space-y-2'>
                  <label className='text-xs font-semibold'>Metadata JSON</label>
                  <Input
                    value={metadataStr}
                    onChange={(event) => setMetadataStr(event.target.value)}
                    placeholder='{}'
                  />
                </div>

                <Button type='submit' disabled={loading}>
                  {loading ? 'Creating…' : 'Create return decision'}
                </Button>
              </form>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className='text-base'>Decision result</CardTitle>
              <CardDescription>
                GraphQL response summary for operator handoff.
              </CardDescription>
            </CardHeader>
            <CardContent className='space-y-3 text-sm'>
              {!result && (
                <p className='text-muted-foreground'>
                  No decision has been created in this session.
                </p>
              )}
              {result && (
                <>
                  <div className='flex items-center justify-between gap-3'>
                    <span className='text-muted-foreground'>Action</span>
                    <Badge>{result.action}</Badge>
                  </div>
                  <div className='flex items-center justify-between gap-3'>
                    <span className='text-muted-foreground'>Return status</span>
                    <Badge variant='secondary'>
                      {result.orderReturn.status}
                    </Badge>
                  </div>
                  <div className='font-mono text-xs break-all'>
                    Return: {result.orderReturn.id}
                  </div>
                  {result.refund && (
                    <div className='font-mono text-xs break-all'>
                      Refund: {result.refund.id}
                    </div>
                  )}
                  {result.orderChange && (
                    <div className='font-mono text-xs break-all'>
                      Order change: {result.orderChange.id}
                    </div>
                  )}
                  <pre className='bg-muted max-h-64 overflow-auto rounded-md p-3 text-xs'>
                    {result.metadata}
                  </pre>
                </>
              )}
            </CardContent>
          </Card>
        </div>
      </div>
    </PageContainer>
  );
}
