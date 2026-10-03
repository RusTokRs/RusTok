'use client';

import * as React from 'react';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger
} from '@/shared/ui/shadcn/dialog';
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import { Switch } from '@/shared/ui/shadcn/switch';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import { Plus, Loader2 } from 'lucide-react';
import { toast } from 'sonner';
import type { CreateProductAttributePayload } from '../../api/types';

interface AttributeCreateDialogProps {
  onCreateAttribute: (payload: CreateProductAttributePayload) => Promise<void>;
}

const VALUE_TYPES = [
  { value: 'text', label: 'Text (Single line)' },
  { value: 'rich_text', label: 'Rich Text (Formatted)' },
  { value: 'integer', label: 'Integer (Whole number)' },
  { value: 'decimal', label: 'Decimal (Floating number)' },
  { value: 'boolean', label: 'Boolean (Yes/No)' },
  { value: 'date', label: 'Date' },
  { value: 'datetime', label: 'Date & Time' },
  { value: 'option', label: 'Single Option (Select dropdown)' },
  { value: 'multi_option', label: 'Multi Option (Checklist)' },
  { value: 'json', label: 'JSON (Raw structured)' }
];

export function AttributeCreateDialog({
  onCreateAttribute
}: AttributeCreateDialogProps) {
  const [open, setOpen] = React.useState(false);
  const [isSubmitting, setIsSubmitting] = React.useState(false);

  const [label, setLabel] = React.useState('');
  const [code, setCode] = React.useState('');
  const [valueType, setValueType] = React.useState('text');
  const [helpText, setHelpText] = React.useState('');
  const [isLocalized, setIsLocalized] = React.useState(false);
  const [isFilterable, setIsFilterable] = React.useState(true);
  const [isSearchable, setIsSearchable] = React.useState(true);
  const [isSortable, setIsSortable] = React.useState(false);
  const [showOnStorefront, setShowOnStorefront] = React.useState(true);

  const handleLabelChange = (val: string) => {
    setLabel(val);
    const generatedCode = val
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9]+/g, '_')
      .replace(/^_+|_+$/g, '');
    if (
      !code ||
      code ===
        val
          .slice(0, -1)
          .toLowerCase()
          .replace(/[^a-z0-9]+/g, '_')
    ) {
      setCode(generatedCode);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!label.trim()) {
      toast.error('Attribute label is required');
      return;
    }
    if (!code.trim()) {
      toast.error('Attribute code is required');
      return;
    }

    setIsSubmitting(true);
    try {
      await onCreateAttribute({
        label: label.trim(),
        code: code.trim(),
        valueType,
        helpText: helpText.trim() || undefined,
        isLocalized,
        isFilterable,
        isSearchable,
        isSortable,
        showOnStorefront
      });
      toast.success('Attribute created successfully');
      setOpen(false);
      setLabel('');
      setCode('');
      setValueType('text');
      setHelpText('');
      setIsLocalized(false);
      setIsFilterable(true);
      setIsSearchable(true);
      setIsSortable(false);
      setShowOnStorefront(true);
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : 'Failed to create attribute'
      );
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button>
          <Plus className='mr-1.5 h-4 w-4' />
          Add Attribute
        </Button>
      </DialogTrigger>
      <DialogContent className='sm:max-w-[550px]'>
        <form onSubmit={handleSubmit}>
          <DialogHeader>
            <DialogTitle>Create Product Attribute</DialogTitle>
            <DialogDescription>
              Define a typed attribute for catalog products and category
              schemas.
            </DialogDescription>
          </DialogHeader>

          <div className='grid gap-4 py-4'>
            <div className='grid grid-cols-2 gap-3'>
              <div className='space-y-1.5'>
                <Label htmlFor='attr-label'>Label *</Label>
                <Input
                  id='attr-label'
                  value={label}
                  onChange={(e) => handleLabelChange(e.target.value)}
                  placeholder='e.g. Switch Type'
                  required
                  disabled={isSubmitting}
                />
              </div>

              <div className='space-y-1.5'>
                <Label htmlFor='attr-code'>Code *</Label>
                <Input
                  id='attr-code'
                  value={code}
                  onChange={(e) => setCode(e.target.value)}
                  placeholder='e.g. switch_type'
                  required
                  disabled={isSubmitting}
                />
              </div>
            </div>

            <div className='space-y-1.5'>
              <Label>Value Type</Label>
              <Select
                value={valueType}
                onValueChange={setValueType}
                disabled={isSubmitting}
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {VALUE_TYPES.map((t) => (
                    <SelectItem key={t.value} value={t.value}>
                      {t.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div className='space-y-1.5'>
              <Label htmlFor='attr-help'>Help / Guideline text</Label>
              <Input
                id='attr-help'
                value={helpText}
                onChange={(e) => setHelpText(e.target.value)}
                placeholder='Optional tooltip or operator guidance'
                disabled={isSubmitting}
              />
            </div>

            <div className='bg-muted/20 space-y-3 rounded-md border p-3'>
              <p className='text-muted-foreground text-xs font-semibold tracking-wide uppercase'>
                Behavior & Visibility
              </p>
              <div className='grid grid-cols-2 gap-3'>
                <div className='flex items-center justify-between'>
                  <Label
                    htmlFor='is-localized'
                    className='cursor-pointer text-xs font-normal'
                  >
                    Localized values
                  </Label>
                  <Switch
                    id='is-localized'
                    checked={isLocalized}
                    onCheckedChange={setIsLocalized}
                    disabled={isSubmitting}
                  />
                </div>

                <div className='flex items-center justify-between'>
                  <Label
                    htmlFor='is-filterable'
                    className='cursor-pointer text-xs font-normal'
                  >
                    Filterable (Facets)
                  </Label>
                  <Switch
                    id='is-filterable'
                    checked={isFilterable}
                    onCheckedChange={setIsFilterable}
                    disabled={isSubmitting}
                  />
                </div>

                <div className='flex items-center justify-between'>
                  <Label
                    htmlFor='is-searchable'
                    className='cursor-pointer text-xs font-normal'
                  >
                    Searchable (Index)
                  </Label>
                  <Switch
                    id='is-searchable'
                    checked={isSearchable}
                    onCheckedChange={setIsSearchable}
                    disabled={isSubmitting}
                  />
                </div>

                <div className='flex items-center justify-between'>
                  <Label
                    htmlFor='is-sortable'
                    className='cursor-pointer text-xs font-normal'
                  >
                    Sortable
                  </Label>
                  <Switch
                    id='is-sortable'
                    checked={isSortable}
                    onCheckedChange={setIsSortable}
                    disabled={isSubmitting}
                  />
                </div>

                <div className='col-span-2 flex items-center justify-between border-t pt-1'>
                  <Label
                    htmlFor='show-storefront'
                    className='cursor-pointer text-xs font-normal'
                  >
                    Show on Storefront specifications
                  </Label>
                  <Switch
                    id='show-storefront'
                    checked={showOnStorefront}
                    onCheckedChange={setShowOnStorefront}
                    disabled={isSubmitting}
                  />
                </div>
              </div>
            </div>
          </div>

          <DialogFooter>
            <Button
              type='button'
              variant='outline'
              onClick={() => setOpen(false)}
              disabled={isSubmitting}
            >
              Cancel
            </Button>
            <Button type='submit' disabled={isSubmitting}>
              {isSubmitting ? (
                <>
                  <Loader2 className='mr-1.5 h-4 w-4 animate-spin' />
                  Creating...
                </>
              ) : (
                'Create Attribute'
              )}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
