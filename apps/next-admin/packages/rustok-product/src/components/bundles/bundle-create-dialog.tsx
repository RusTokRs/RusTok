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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import { Loader2 } from 'lucide-react';
import type { CreateBundleInput } from '../../api/types';
import { isDefinitiveProductWriteRefusal } from '../../api/write-outcome';

interface BundleCreateDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreate: (input: CreateBundleInput, idempotencyKey: string) => Promise<void>;
  disabled?: boolean;
}

export function BundleCreateDialog({
  open,
  onOpenChange,
  onCreate,
  disabled
}: BundleCreateDialogProps) {
  const [name, setName] = React.useState('');
  const [slug, setSlug] = React.useState('');
  const [bundleType, setBundleType] = React.useState('fixed');
  const [status, setStatus] = React.useState('active');
  const [discountType, setDiscountType] = React.useState('none');
  const [discountValue, setDiscountValue] = React.useState('0');
  const [isSubmitting, setIsSubmitting] = React.useState(false);
  // One idempotency key per submitted intent: an ambiguous (network-level)
  // failure keeps it so a retry replays the owner receipt instead of creating
  // a duplicate bundle; confirmed or definitively refused attempts rotate it.
  const createAttemptKey = React.useRef<string | null>(null);

  const resetCreateAttempt = () => {
    createAttemptKey.current = null;
  };

  React.useEffect(() => {
    if (open) {
      setName('');
      setSlug('');
      setBundleType('fixed');
      setStatus('active');
      setDiscountType('none');
      setDiscountValue('0');
      resetCreateAttempt();
    }
  }, [open]);

  const handleNameChange = (val: string) => {
    resetCreateAttempt();
    setName(val);
    const autoSlug = val
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-|-$/g, '');
    setSlug(autoSlug);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim() || !slug.trim()) return;

    setIsSubmitting(true);
    createAttemptKey.current ??= crypto.randomUUID();
    try {
      await onCreate(
        {
          name: name.trim(),
          slug: slug.trim(),
          bundleType,
          status,
          discountType,
          discountValue
        },
        createAttemptKey.current
      );
      resetCreateAttempt();
      onOpenChange(false);
    } catch (err) {
      if (isDefinitiveProductWriteRefusal(err)) resetCreateAttempt();
      toast.error(
        err instanceof Error ? err.message : 'Failed to create bundle.'
      );
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className='sm:max-w-[480px]'>
        <form onSubmit={handleSubmit}>
          <DialogHeader>
            <DialogTitle>Create New Bundle</DialogTitle>
            <DialogDescription>
              Define a new product package, gift set, or configurable kit.
            </DialogDescription>
          </DialogHeader>

          <div className='space-y-4 py-4'>
            <div className='space-y-1.5'>
              <Label htmlFor='bundle-create-name'>Bundle Name *</Label>
              <Input
                id='bundle-create-name'
                placeholder='e.g. Premium Starter Kit'
                value={name}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                  handleNameChange(e.target.value)
                }
                required
                disabled={isSubmitting || disabled}
              />
            </div>

            <div className='space-y-1.5'>
              <Label htmlFor='bundle-create-slug'>Slug *</Label>
              <Input
                id='bundle-create-slug'
                placeholder='e.g. premium-starter-kit'
                value={slug}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                  resetCreateAttempt();
                  setSlug(e.target.value);
                }}
                required
                disabled={isSubmitting || disabled}
              />
            </div>

            <div className='grid grid-cols-2 gap-4'>
              <div className='space-y-1.5'>
                <Label>Kit Type</Label>
                <Select
                  value={bundleType}
                  onValueChange={(value) => {
                    resetCreateAttempt();
                    setBundleType(value);
                  }}
                  disabled={isSubmitting || disabled}
                >
                  <SelectTrigger className='h-9'>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value='fixed'>Fixed Composition</SelectItem>
                    <SelectItem value='flexible'>
                      Flexible / Configurable
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <div className='space-y-1.5'>
                <Label>Status</Label>
                <Select
                  value={status}
                  onValueChange={(value) => {
                    resetCreateAttempt();
                    setStatus(value);
                  }}
                  disabled={isSubmitting || disabled}
                >
                  <SelectTrigger className='h-9'>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value='active'>Active</SelectItem>
                    <SelectItem value='draft'>Draft</SelectItem>
                    <SelectItem value='archived'>Archived</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>

            <div className='grid grid-cols-2 gap-4'>
              <div className='space-y-1.5'>
                <Label>Discount Type</Label>
                <Select
                  value={discountType}
                  onValueChange={(value) => {
                    resetCreateAttempt();
                    setDiscountType(value);
                  }}
                  disabled={isSubmitting || disabled}
                >
                  <SelectTrigger className='h-9'>
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
                <Label>Discount Value</Label>
                <Input
                  type='number'
                  min={0}
                  step={discountType === 'percentage' ? 0.5 : 0.01}
                  value={discountValue}
                  onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                    resetCreateAttempt();
                    setDiscountValue(e.target.value);
                  }}
                  disabled={discountType === 'none' || isSubmitting || disabled}
                  className='h-9'
                />
              </div>
            </div>
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
              disabled={
                !name.trim() || !slug.trim() || isSubmitting || disabled
              }
            >
              {isSubmitting ? (
                <>
                  <Loader2 className='mr-2 h-4 w-4 animate-spin' />
                  Creating...
                </>
              ) : (
                'Create Bundle'
              )}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
