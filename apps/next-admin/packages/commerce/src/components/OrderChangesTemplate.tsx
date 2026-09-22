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
import {
  listOrderChanges,
  applyOrderChange,
  cancelOrderChange,
  GqlOpts
} from '../api';
import type { OrderChange } from '../types';

export function OrderChangesTemplate({ opts }: { opts: GqlOpts }) {
  const [changes, setChanges] = React.useState<OrderChange[]>([]);
  const [total, setTotal] = React.useState(0);
  const [page, setPage] = React.useState(1);
  const [hasNext, setHasNext] = React.useState(false);
  const [loading, setLoading] = React.useState(true);
  const [error, setError] = React.useState<string | null>(null);
  const [feedback, setFeedback] = React.useState<string | null>(null);

  // Filters
  const [orderIdFilter, setOrderIdFilter] = React.useState('');
  const [statusFilter, setStatusFilter] = React.useState('');
  const [changeTypeFilter, setChangeTypeFilter] = React.useState('');

  // Form states for Apply/Cancel
  const [selectedChange, setSelectedChange] =
    React.useState<OrderChange | null>(null);
  const [formAction, setFormAction] = React.useState<'apply' | 'cancel' | null>(
    null
  );

  // Apply exchange fields
  const [diffAmount, setDiffAmount] = React.useState('');
  const [diffReason, setDiffReason] = React.useState('');

  // Cancel fields
  const [cancelReason, setCancelReason] = React.useState('');

  const loadData = React.useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await listOrderChanges(opts, {
        page,
        perPage: 10,
        orderId: orderIdFilter.trim() || undefined,
        status: statusFilter || undefined,
        changeType: changeTypeFilter || undefined
      });
      setChanges(result.items);
      setTotal(result.total);
      setHasNext(result.hasNext);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : 'Failed to load order changes.'
      );
    } finally {
      setLoading(false);
    }
  }, [opts, page, orderIdFilter, statusFilter, changeTypeFilter]);

  React.useEffect(() => {
    void loadData();
  }, [loadData]);

  const handleApplyClick = (change: OrderChange) => {
    setSelectedChange(change);
    setFormAction('apply');
    setDiffAmount('');
    setDiffReason('');
  };

  const handleCancelClick = (change: OrderChange) => {
    setSelectedChange(change);
    setFormAction('cancel');
    setCancelReason('');
  };

  const handleSubmitAction = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedChange) return;
    setError(null);
    setFeedback(null);
    setLoading(true);

    try {
      if (formAction === 'apply') {
        const differenceRefund = diffAmount.trim()
          ? {
              amount: diffAmount.trim(),
              reason: diffReason.trim() || null,
              metadata: null
            }
          : null;

        await applyOrderChange(opts, selectedChange.id, null, differenceRefund);
        setFeedback(`Order change ${selectedChange.id} applied successfully.`);
      } else if (formAction === 'cancel') {
        await cancelOrderChange(
          opts,
          selectedChange.id,
          cancelReason.trim() || null,
          null
        );
        setFeedback(
          `Order change ${selectedChange.id} cancelled successfully.`
        );
      }

      setFormAction(null);
      setSelectedChange(null);
      void loadData();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Operation failed.');
    } finally {
      setLoading(false);
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'applied':
        return 'default';
      case 'pending':
        return 'secondary';
      case 'cancelled':
        return 'destructive';
      default:
        return 'outline';
    }
  };

  return (
    <PageContainer
      pageTitle='Order Changes'
      pageDescription='Inspect, apply, or cancel returns, exchanges, and claims.'
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

        {/* Action Form Panel */}
        {selectedChange && formAction && (
          <Card className='border-primary/20 bg-primary/5'>
            <CardHeader>
              <CardTitle className='text-base'>
                {formAction === 'apply'
                  ? 'Apply Order Change'
                  : 'Cancel Order Change'}
              </CardTitle>
              <CardDescription>
                Confirm action for order change <b>{selectedChange.id}</b> (
                {selectedChange.changeType}).
              </CardDescription>
            </CardHeader>
            <CardContent>
              <form onSubmit={handleSubmitAction} className='space-y-4'>
                {formAction === 'apply' &&
                  selectedChange.changeType === 'exchange' && (
                    <div className='bg-background space-y-3 rounded border p-3'>
                      <h4 className='text-muted-foreground text-xs font-bold uppercase'>
                        Exchange Difference Refund
                      </h4>
                      <div className='grid gap-4 md:grid-cols-2'>
                        <div className='space-y-2'>
                          <label className='text-xs font-semibold'>
                            Refund Amount (Optional)
                          </label>
                          <Input
                            placeholder='e.g. 15.00 (leave empty for auto calculation)'
                            value={diffAmount}
                            onChange={(e) => setDiffAmount(e.target.value)}
                          />
                        </div>
                        <div className='space-y-2'>
                          <label className='text-xs font-semibold'>
                            Refund Reason (Optional)
                          </label>
                          <Input
                            placeholder='Exchange price difference refund'
                            value={diffReason}
                            onChange={(e) => setDiffReason(e.target.value)}
                          />
                        </div>
                      </div>
                    </div>
                  )}

                {formAction === 'cancel' && (
                  <div className='space-y-2'>
                    <label className='text-xs font-semibold'>
                      Cancellation Reason (Optional)
                    </label>
                    <Input
                      placeholder='Operator requested cancellation'
                      value={cancelReason}
                      onChange={(e) => setCancelReason(e.target.value)}
                    />
                  </div>
                )}

                <div className='flex gap-2'>
                  <Button type='submit'>
                    {formAction === 'apply' ? 'Apply Change' : 'Cancel Change'}
                  </Button>
                  <Button
                    type='button'
                    variant='outline'
                    onClick={() => {
                      setFormAction(null);
                      setSelectedChange(null);
                    }}
                  >
                    Cancel
                  </Button>
                </div>
              </form>
            </CardContent>
          </Card>
        )}

        {/* Filters Card */}
        <Card>
          <CardHeader>
            <CardTitle className='text-base'>Filters</CardTitle>
          </CardHeader>
          <CardContent className='grid items-end gap-4 md:grid-cols-4'>
            <div className='space-y-2'>
              <label className='text-xs font-semibold'>Order ID</label>
              <Input
                placeholder='Search Order UUID...'
                value={orderIdFilter}
                onChange={(e) => setOrderIdFilter(e.target.value)}
              />
            </div>
            <div className='space-y-2'>
              <label className='text-xs font-semibold'>Status</label>
              <select
                className='border-input bg-background w-full rounded-md border px-3 py-2 text-sm shadow-sm'
                value={statusFilter}
                onChange={(e) => setStatusFilter(e.target.value)}
              >
                <option value=''>All Statuses</option>
                <option value='pending'>Pending</option>
                <option value='applied'>Applied</option>
                <option value='cancelled'>Cancelled</option>
              </select>
            </div>
            <div className='space-y-2'>
              <label className='text-xs font-semibold'>Change Type</label>
              <select
                className='border-input bg-background w-full rounded-md border px-3 py-2 text-sm shadow-sm'
                value={changeTypeFilter}
                onChange={(e) => setChangeTypeFilter(e.target.value)}
              >
                <option value=''>All Types</option>
                <option value='return'>Return</option>
                <option value='exchange'>Exchange</option>
                <option value='claim'>Claim</option>
              </select>
            </div>
            <Button onClick={loadData}>Apply Filters</Button>
          </CardContent>
        </Card>

        {/* List Card */}
        <Card>
          <CardHeader>
            <CardTitle className='text-base'>
              Order Change Records ({total})
            </CardTitle>
          </CardHeader>
          <CardContent>
            {loading ? (
              <div className='text-muted-foreground animate-pulse py-8 text-center text-sm'>
                Loading order changes...
              </div>
            ) : (
              <div className='rounded-md border'>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Change ID</TableHead>
                      <TableHead>Order ID</TableHead>
                      <TableHead>Type</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead>Description</TableHead>
                      <TableHead className='text-right'>Actions</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {changes.length === 0 ? (
                      <TableRow>
                        <TableCell
                          colSpan={6}
                          className='text-muted-foreground py-6 text-center text-sm'
                        >
                          No order changes found matching filters.
                        </TableCell>
                      </TableRow>
                    ) : (
                      changes.map((change) => (
                        <TableRow key={change.id}>
                          <TableCell className='font-mono text-xs font-medium'>
                            {change.id}
                          </TableCell>
                          <TableCell className='text-muted-foreground font-mono text-xs'>
                            {change.orderId}
                          </TableCell>
                          <TableCell>
                            <Badge variant='outline' className='capitalize'>
                              {change.changeType}
                            </Badge>
                          </TableCell>
                          <TableCell>
                            <Badge variant={getStatusColor(change.status)}>
                              {change.status}
                            </Badge>
                          </TableCell>
                          <TableCell className='max-w-xs truncate'>
                            {change.description || '-'}
                          </TableCell>
                          <TableCell className='flex items-center justify-end gap-2'>
                            {change.status === 'pending' && (
                              <>
                                <Button
                                  size='sm'
                                  onClick={() => handleApplyClick(change)}
                                >
                                  Apply
                                </Button>
                                <Button
                                  size='sm'
                                  variant='destructive'
                                  onClick={() => handleCancelClick(change)}
                                >
                                  Cancel
                                </Button>
                              </>
                            )}
                          </TableCell>
                        </TableRow>
                      ))
                    )}
                  </TableBody>
                </Table>
              </div>
            )}

            <div className='mt-4 flex items-center justify-end gap-2'>
              <Button
                size='sm'
                variant='outline'
                disabled={page <= 1}
                onClick={() => setPage((p) => p - 1)}
              >
                Previous
              </Button>
              <Button
                size='sm'
                variant='outline'
                disabled={!hasNext}
                onClick={() => setPage((p) => p + 1)}
              >
                Next
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>
    </PageContainer>
  );
}
