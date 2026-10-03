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
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import { Textarea } from '@/shared/ui/shadcn/textarea';
import { Switch } from '@/shared/ui/shadcn/switch';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Loader2, FolderTree, AlertCircle, Layers } from 'lucide-react';
import type {
  CatalogCategorySummary,
  ProductEffectiveForm,
  ProductAttributeValuePatch
} from '../../api/types';

interface ProductCategoryCardProps {
  categories: CatalogCategorySummary[];
  selectedCategoryId: string;
  onSelectCategory: (categoryId: string) => void;
  effectiveForm: ProductEffectiveForm | null;
  isLoadingForm: boolean;
  attributeValues: Record<string, ProductAttributeValuePatch>;
  onAttributeValueChange: (
    attributeId: string,
    patch: Partial<ProductAttributeValuePatch>
  ) => void;
  disabled?: boolean;
}

export function ProductCategoryCard({
  categories,
  selectedCategoryId,
  onSelectCategory,
  effectiveForm,
  isLoadingForm,
  attributeValues,
  onAttributeValueChange,
  disabled = false
}: ProductCategoryCardProps) {
  // Build category display items with path or tree indent
  const categoryOptions = React.useMemo(() => {
    return categories.map((cat) => ({
      id: cat.id,
      label: cat.path || cat.name || cat.code,
      kind: cat.kind
    }));
  }, [categories]);

  // Group attributes by groupCode / groupLabel
  const groupedAttributes = React.useMemo(() => {
    if (!effectiveForm || !effectiveForm.attributes) return {};
    const groups: Record<
      string,
      { label: string; attributes: typeof effectiveForm.attributes }
    > = {};

    for (const attr of effectiveForm.attributes) {
      const gCode = attr.groupCode || 'general';
      const gLabel = attr.groupLabel || 'General Specifications';
      if (!groups[gCode]) {
        groups[gCode] = { label: gLabel, attributes: [] };
      }
      groups[gCode].attributes.push(attr);
    }
    return groups;
  }, [effectiveForm]);

  return (
    <Card className='border-border rounded-2xl shadow-sm'>
      <CardHeader className='border-border/60 border-b pb-4'>
        <div className='flex items-center gap-2'>
          <FolderTree className='text-primary h-4 w-4' />
          <div>
            <CardTitle className='text-sm font-semibold'>
              Category & Specifications
            </CardTitle>
            <CardDescription className='text-xs'>
              Select a primary category to inherit and edit schema-defined
              product attributes.
            </CardDescription>
          </div>
        </div>
      </CardHeader>
      <CardContent className='space-y-6 pt-5'>
        {/* Category Picker */}
        <div className='space-y-1.5'>
          <Label htmlFor='category-select' className='text-xs font-medium'>
            Primary Category
          </Label>
          <Select
            value={selectedCategoryId || 'none'}
            onValueChange={(val) => onSelectCategory(val === 'none' ? '' : val)}
            disabled={disabled}
          >
            <SelectTrigger
              id='category-select'
              className='h-9 rounded-xl text-xs'
            >
              <SelectValue placeholder='Select a category...' />
            </SelectTrigger>
            <SelectContent className='max-h-64'>
              <SelectItem
                value='none'
                className='text-muted-foreground text-xs'
              >
                No category assigned
              </SelectItem>
              {categoryOptions.map((opt) => (
                <SelectItem key={opt.id} value={opt.id} className='text-xs'>
                  <span className='font-medium'>{opt.label}</span>
                  <span className='text-muted-foreground ml-2 text-[10px]'>
                    ({opt.kind})
                  </span>
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        {/* Dynamic Effective Form */}
        <div className='border-border/40 border-t pt-2'>
          {isLoadingForm ? (
            <div className='text-muted-foreground flex items-center justify-center gap-2 py-8 text-xs'>
              <Loader2 className='text-primary h-4 w-4 animate-spin' />
              <span>Loading category attribute form...</span>
            </div>
          ) : !selectedCategoryId ? (
            <div className='text-muted-foreground bg-muted/20 border-border/80 flex flex-col items-center justify-center gap-2 rounded-xl border border-dashed py-8 text-center'>
              <Layers className='h-6 w-6 opacity-40' />
              <p className='text-xs font-medium'>
                No primary category selected
              </p>
              <p className='max-w-xs text-[11px] opacity-75'>
                Choose a category above to load typed attributes and facet
                specifications.
              </p>
            </div>
          ) : Object.keys(groupedAttributes).length === 0 ? (
            <div className='text-muted-foreground bg-muted/20 rounded-xl py-6 text-center text-xs italic'>
              This category has no schema-assigned attributes yet.
            </div>
          ) : (
            <div className='space-y-6'>
              {Object.entries(groupedAttributes).map(([gCode, group]) => (
                <div key={gCode} className='space-y-3.5'>
                  <div className='border-border/50 flex items-center gap-2 border-b pb-1.5'>
                    <h4 className='text-foreground text-xs font-semibold tracking-wider uppercase'>
                      {group.label}
                    </h4>
                    <span className='text-muted-foreground text-[10px]'>
                      ({group.attributes.length})
                    </span>
                  </div>

                  <div className='grid grid-cols-1 gap-4 sm:grid-cols-2'>
                    {group.attributes.map((attr) => {
                      const val = attributeValues[attr.attributeId] || {
                        attributeId: attr.attributeId,
                        kind: attr.valueType.toUpperCase()
                      };

                      return (
                        <div key={attr.attributeId} className='space-y-1.5'>
                          <div className='flex items-center justify-between'>
                            <Label
                              htmlFor={`attr-${attr.attributeId}`}
                              className='flex items-center gap-1 text-xs font-medium'
                            >
                              <span>{attr.label}</span>
                              {attr.isRequired && (
                                <span className='text-destructive'>*</span>
                              )}
                            </Label>
                            <span className='text-muted-foreground/60 font-mono text-[10px]'>
                              {attr.code}
                            </span>
                          </div>

                          {/* Render control by valueType */}
                          {renderAttributeControl(
                            attr,
                            val,
                            (patch) =>
                              onAttributeValueChange(attr.attributeId, patch),
                            disabled
                          )}
                        </div>
                      );
                    })}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

function renderAttributeControl(
  attr: ProductEffectiveForm['attributes'][number],
  val: ProductAttributeValuePatch,
  onChange: (patch: Partial<ProductAttributeValuePatch>) => void,
  disabled: boolean
) {
  const type = attr.valueType.toLowerCase();

  switch (type) {
    case 'boolean':
      return (
        <div className='flex h-9 items-center gap-2'>
          <Switch
            id={`attr-${attr.attributeId}`}
            checked={Boolean(val.boolean)}
            onCheckedChange={(checked) =>
              onChange({ kind: 'BOOLEAN', boolean: checked })
            }
            disabled={disabled}
          />
          <span className='text-muted-foreground text-xs font-medium'>
            {val.boolean ? 'Yes' : 'No'}
          </span>
        </div>
      );

    case 'integer':
      return (
        <Input
          id={`attr-${attr.attributeId}`}
          type='number'
          step='1'
          value={
            val.integer !== undefined && val.integer !== null ? val.integer : ''
          }
          onChange={(e) => {
            const parsed =
              e.target.value === '' ? null : parseInt(e.target.value, 10);
            onChange({
              kind: 'INTEGER',
              integer: isNaN(parsed as number) ? null : parsed
            });
          }}
          placeholder='0'
          className='h-9 rounded-xl font-mono text-xs'
          disabled={disabled}
        />
      );

    case 'decimal':
      return (
        <Input
          id={`attr-${attr.attributeId}`}
          type='number'
          step='any'
          value={val.decimal || ''}
          onChange={(e) =>
            onChange({ kind: 'DECIMAL', decimal: e.target.value || null })
          }
          placeholder='0.00'
          className='h-9 rounded-xl font-mono text-xs'
          disabled={disabled}
        />
      );

    case 'date':
      return (
        <Input
          id={`attr-${attr.attributeId}`}
          type='date'
          value={val.date || ''}
          onChange={(e) =>
            onChange({ kind: 'DATE', date: e.target.value || null })
          }
          className='h-9 rounded-xl text-xs'
          disabled={disabled}
        />
      );

    case 'datetime':
      return (
        <Input
          id={`attr-${attr.attributeId}`}
          type='datetime-local'
          value={val.datetime ? val.datetime.slice(0, 16) : ''}
          onChange={(e) =>
            onChange({
              kind: 'DATETIME',
              datetime: e.target.value
                ? new Date(e.target.value).toISOString()
                : null
            })
          }
          className='h-9 rounded-xl text-xs'
          disabled={disabled}
        />
      );

    case 'option':
      return (
        <Select
          value={val.optionId || 'none'}
          onValueChange={(optId) =>
            onChange({
              kind: 'OPTION',
              optionId: optId === 'none' ? null : optId
            })
          }
          disabled={disabled}
        >
          <SelectTrigger
            id={`attr-${attr.attributeId}`}
            className='h-9 rounded-xl text-xs'
          >
            <SelectValue placeholder='Select option...' />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value='none' className='text-muted-foreground text-xs'>
              Not selected
            </SelectItem>
            {attr.options.map((opt) => (
              <SelectItem key={opt.id} value={opt.id} className='text-xs'>
                {opt.label} ({opt.code})
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      );

    case 'multi_option':
      const currentSelected = val.optionIds || [];
      return (
        <div className='border-input bg-background flex min-h-[42px] flex-wrap gap-1.5 rounded-xl border p-2'>
          {attr.options.map((opt) => {
            const isChecked = currentSelected.includes(opt.id);
            return (
              <button
                key={opt.id}
                type='button'
                disabled={disabled}
                onClick={() => {
                  const updated = isChecked
                    ? currentSelected.filter((id) => id !== opt.id)
                    : [...currentSelected, opt.id];
                  onChange({ kind: 'MULTI_OPTION', optionIds: updated });
                }}
                className={`rounded-lg border px-2.5 py-1 text-xs transition ${
                  isChecked
                    ? 'bg-primary text-primary-foreground border-primary font-semibold'
                    : 'bg-muted/40 text-foreground border-border hover:bg-muted'
                }`}
              >
                {opt.label}
              </button>
            );
          })}
        </div>
      );

    case 'json':
      return (
        <Textarea
          id={`attr-${attr.attributeId}`}
          rows={2}
          value={val.json || ''}
          onChange={(e) =>
            onChange({ kind: 'JSON', json: e.target.value || null })
          }
          placeholder='{"key": "value"}'
          className='rounded-xl font-mono text-xs'
          disabled={disabled}
        />
      );

    case 'rich_text':
      return (
        <Textarea
          id={`attr-${attr.attributeId}`}
          rows={3}
          value={val.text || ''}
          onChange={(e) =>
            onChange({ kind: 'TEXT', text: e.target.value || null })
          }
          placeholder='Enter formatted text...'
          className='rounded-xl text-xs'
          disabled={disabled}
        />
      );

    case 'text':
    default:
      return (
        <Input
          id={`attr-${attr.attributeId}`}
          value={val.text || ''}
          onChange={(e) =>
            onChange({ kind: 'TEXT', text: e.target.value || null })
          }
          placeholder='Attribute value...'
          className='h-9 rounded-xl text-xs'
          disabled={disabled}
        />
      );
  }
}
