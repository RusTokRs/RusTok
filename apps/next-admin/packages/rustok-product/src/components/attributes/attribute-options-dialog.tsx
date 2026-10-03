'use client';

import * as React from 'react';
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
import { Loader2 } from 'lucide-react';
import { toast } from 'sonner';
import type { ProductAttributeSummary, CreateProductAttributeOptionPayload } from '../../api/types';

interface AttributeOptionsDialogProps {
  attribute: ProductAttributeSummary | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreateOption: (payload: CreateProductAttributeOptionPayload) => Promise<void>;
}

export function AttributeOptionsDialog({
  attribute,
  open,
  onOpenChange,
  onCreateOption
}: AttributeOptionsDialogProps) {
  const [isSubmitting, setIsSubmitting] = React.useState(false);
  const [label, setLabel] = React.useState('');
  const [code, setCode] = React.useState('');
  const [position, setPosition] = React.useState<number>(0);

  const handleLabelChange = (val: string) => {
    setLabel(val);
    const generatedCode = val
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9]+/g, '_')
      .replace(/^_+|_+$/g, '');
    if (!code || code === val.slice(0, -1).toLowerCase().replace(/[^a-z0-9]+/g, '_')) {
      setCode(generatedCode);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!attribute) return;
    if (!label.trim()) {
      toast.error('Option label is required');
      return;
    }
    if (!code.trim()) {
      toast.error('Option code is required');
      return;
    }

    setIsSubmitting(true);
    try {
      await onCreateOption({
        attributeId: attribute.id,
        label: label.trim(),
        code: code.trim(),
        position
      });
      toast.success(`Option "${label}" added to ${attribute.label}`);
      setLabel('');
      setCode('');
      setPosition((prev) => prev + 1);
      onOpenChange(false);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to add option');
    } finally {
      setIsSubmitting(false);
    }
  };

  if (!attribute) return null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className='sm:max-w-[450px]'>
        <form onSubmit={handleSubmit}>
          <DialogHeader>
            <DialogTitle>Add Option to {attribute.label}</DialogTitle>
            <DialogDescription>
              Add a selectable dictionary option for attribute{' '}
              <code className='bg-muted rounded px-1 text-xs'>{attribute.code}</code>.
            </DialogDescription>
          </DialogHeader>

          <div className='grid gap-4 py-4'>
            <div className='space-y-1.5'>
              <Label htmlFor='opt-label'>Option Label *</Label>
              <Input
                id='opt-label'
                value={label}
                onChange={(e) => handleLabelChange(e.target.value)}
                placeholder='e.g. Red, XL, Cherry MX Brown'
                required
                disabled={isSubmitting}
              />
            </div>

            <div className='grid grid-cols-2 gap-3'>
              <div className='space-y-1.5'>
                <Label htmlFor='opt-code'>Option Code *</Label>
                <Input
                  id='opt-code'
                  value={code}
                  onChange={(e) => setCode(e.target.value)}
                  placeholder='e.g. red, xl, cherry_mx_brown'
                  required
                  disabled={isSubmitting}
                />
              </div>

              <div className='space-y-1.5'>
                <Label htmlFor='opt-pos'>Position</Label>
                <Input
                  id='opt-pos'
                  type='number'
                  value={position}
                  onChange={(e) => setPosition(Number(e.target.value) || 0)}
                  disabled={isSubmitting}
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
            <Button type='submit' disabled={isSubmitting}>
              {isSubmitting ? (
                <>
                  <Loader2 className='mr-1.5 h-4 w-4 animate-spin' />
                  Adding...
                </>
              ) : (
                'Add Option'
              )}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
