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
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import { Button } from '@/shared/ui/shadcn/button';
import { Label } from '@/shared/ui/shadcn/label';
import { Badge } from '@/shared/ui/shadcn/badge';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import { ArrowDown, ArrowUp, Plus, Save, Trash2 } from 'lucide-react';
import type {
  ProductEffectiveForm,
  SetVariantAxesInput,
  VariantAxisConfig
} from '../../api/types';

interface AxisOptionDraft {
  id: string;
  label: string;
}

interface AxisRowDraft {
  attributeId: string;
  code: string;
  label: string;
  allowedOptionIds: string[];
  options: AxisOptionDraft[];
}

interface ProductVariantAxesCardProps {
  axes: VariantAxisConfig[];
  effectiveForm?: ProductEffectiveForm | null;
  onSaveAxes?: (
    input: SetVariantAxesInput
  ) => Promise<VariantAxisConfig[] | void>;
  disabled?: boolean;
}

const SELECT_EMPTY = '__none__';

function shortId(value: string): string {
  return value.length > 8 ? value.slice(0, 8) : value;
}

function buildAxisRow(attributeId: string, form: ProductEffectiveForm | null) {
  const attribute = form?.attributes.find(
    (candidate) => candidate.attributeId === attributeId
  );
  if (!attribute) {
    return null;
  }

  return {
    attributeId: attribute.attributeId,
    code: attribute.code,
    label: attribute.label,
    allowedOptionIds: attribute.options.map((option) => option.id),
    options: attribute.options.map((option) => ({
      id: option.id,
      label: option.label
    }))
  } satisfies AxisRowDraft;
}

function seedAxisRows(
  axes: VariantAxisConfig[],
  form: ProductEffectiveForm | null
): AxisRowDraft[] {
  if (axes.length > 0) {
    return axes.map((axis) => {
      const schemaAttribute = form?.attributes.find(
        (candidate) => candidate.attributeId === axis.attributeId
      );
      const options = schemaAttribute
        ? schemaAttribute.options.map((option) => ({
            id: option.id,
            label: option.label
          }))
        : axis.allowedValues.map((value) => ({
            id: value.optionId,
            label: value.value || shortId(value.optionId)
          }));

      return {
        attributeId: axis.attributeId,
        code: axis.code,
        label: axis.name,
        allowedOptionIds: axis.allowedValues.map((value) => value.optionId),
        options
      };
    });
  }

  if (!form) {
    return [];
  }

  return form.attributes
    .filter(
      (attribute) =>
        attribute.defaultVariantAxis &&
        attribute.variantAxisPolicy !== 'forbidden' &&
        attribute.options.length > 0
    )
    .map((attribute) => ({
      attributeId: attribute.attributeId,
      code: attribute.code,
      label: attribute.label,
      allowedOptionIds: attribute.options.map((option) => option.id),
      options: attribute.options.map((option) => ({
        id: option.id,
        label: option.label
      }))
    }));
}

/**
 * Variant-axis editor for the ADR identity model.
 *
 * The card offers only category-schema attributes whose `variantAxisPolicy` is
 * not `forbidden` and that carry options, seeds itself from the saved axes (or
 * from the schema defaults for a product that has none yet), and persists the
 * ordered axes through the idempotent `setProductVariantAxes` command. Saving an
 * empty configuration clears the axes, which the owner accepts.
 */
export function ProductVariantAxesCard({
  axes,
  effectiveForm = null,
  onSaveAxes,
  disabled = false
}: ProductVariantAxesCardProps) {
  const [rows, setRows] = React.useState<AxisRowDraft[]>(() =>
    seedAxisRows(axes, effectiveForm)
  );
  const [addSelection, setAddSelection] = React.useState(SELECT_EMPTY);
  const [isBusy, setIsBusy] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [notice, setNotice] = React.useState<string | null>(null);

  const availableCandidates = React.useMemo(() => {
    const used = new Set(rows.map((row) => row.attributeId));
    return (effectiveForm?.attributes ?? []).filter(
      (attribute) =>
        attribute.variantAxisPolicy !== 'forbidden' &&
        attribute.options.length > 0 &&
        !used.has(attribute.attributeId)
    );
  }, [rows, effectiveForm]);

  const moveAxis = (index: number, delta: number) => {
    setRows((current) => {
      const target = index + delta;
      if (target < 0 || target >= current.length) {
        return current;
      }
      const next = [...current];
      const [moved] = next.splice(index, 1);
      next.splice(target, 0, moved);
      return next;
    });
  };

  const removeAxis = (attributeId: string) => {
    setRows((current) =>
      current.filter((row) => row.attributeId !== attributeId)
    );
  };

  const toggleOption = (attributeId: string, optionId: string) => {
    setRows((current) =>
      current.map((row) => {
        if (row.attributeId !== attributeId) {
          return row;
        }
        const selected = row.allowedOptionIds.includes(optionId);
        return {
          ...row,
          allowedOptionIds: selected
            ? row.allowedOptionIds.filter((value) => value !== optionId)
            : [...row.allowedOptionIds, optionId]
        };
      })
    );
  };

  const addAxis = () => {
    if (addSelection === SELECT_EMPTY) {
      return;
    }
    const row = buildAxisRow(addSelection, effectiveForm);
    if (!row) {
      return;
    }
    setRows((current) =>
      current.some((existing) => existing.attributeId === row.attributeId)
        ? current
        : [...current, row]
    );
    setAddSelection(SELECT_EMPTY);
  };

  const handleSave = async () => {
    if (!onSaveAxes) {
      return;
    }

    setIsBusy(true);
    setError(null);
    setNotice(null);

    const cleared = rows.length === 0;

    try {
      const saved = await onSaveAxes({
        axes: rows.map((row, index) => ({
          attributeId: row.attributeId,
          position: index,
          allowedOptionIds: row.allowedOptionIds
        }))
      });

      if (Array.isArray(saved)) {
        setRows(seedAxisRows(saved, effectiveForm));
      }
      setNotice(cleared ? 'Variant axes cleared.' : 'Variant axes saved.');
    } catch (failure) {
      setError(
        failure instanceof Error
          ? failure.message
          : 'Variant axes could not be saved.'
      );
    } finally {
      setIsBusy(false);
    }
  };

  return (
    <Card>
      <CardHeader>
        <div className='flex flex-wrap items-start justify-between gap-3'>
          <div className='space-y-1'>
            <CardTitle className='flex items-center gap-2 text-base'>
              Variant axes
              <Badge variant='secondary'>{rows.length}</Badge>
            </CardTitle>
            <CardDescription>
              Axes define the combination identity. Only category-schema
              attributes whose axis policy is not forbidden are offered.
            </CardDescription>
          </div>
          <Button
            type='button'
            size='sm'
            onClick={handleSave}
            disabled={disabled || isBusy || !onSaveAxes}
          >
            <Save className='mr-2 h-4 w-4' />
            {isBusy ? 'Saving...' : 'Save axes'}
          </Button>
        </div>
      </CardHeader>
      <CardContent className='space-y-3'>
        {error ? (
          <p className='border-destructive/40 bg-destructive/10 text-destructive rounded-md border px-3 py-2 text-xs'>
            {error}
          </p>
        ) : null}
        {notice ? (
          <p className='rounded-md border border-emerald-300/50 bg-emerald-500/10 px-3 py-2 text-xs text-emerald-700 dark:text-emerald-400'>
            {notice}
          </p>
        ) : null}

        {rows.length === 0 ? (
          <p className='text-muted-foreground text-xs'>
            No axes configured. Add an attribute to enable variant combinations.
          </p>
        ) : null}

        <div className='space-y-3'>
          {rows.map((row, index) => (
            <div
              key={row.attributeId}
              className='border-border/70 bg-background space-y-2 rounded-lg border p-3'
            >
              <div className='flex flex-wrap items-center justify-between gap-2'>
                <div className='flex items-center gap-2 text-sm font-medium'>
                  <span className='text-muted-foreground'>#{index + 1}</span>
                  <span>{row.label}</span>
                  <span className='text-muted-foreground font-mono text-[11px]'>
                    {row.code}
                  </span>
                </div>
                <div className='flex items-center gap-1'>
                  <Button
                    type='button'
                    variant='outline'
                    size='icon'
                    className='h-7 w-7'
                    disabled={index === 0 || disabled}
                    onClick={() => moveAxis(index, -1)}
                    aria-label='Move axis up'
                  >
                    <ArrowUp className='h-3.5 w-3.5' />
                  </Button>
                  <Button
                    type='button'
                    variant='outline'
                    size='icon'
                    className='h-7 w-7'
                    disabled={index + 1 === rows.length || disabled}
                    onClick={() => moveAxis(index, 1)}
                    aria-label='Move axis down'
                  >
                    <ArrowDown className='h-3.5 w-3.5' />
                  </Button>
                  <Button
                    type='button'
                    variant='outline'
                    size='icon'
                    className='text-destructive h-7 w-7'
                    disabled={disabled}
                    onClick={() => removeAxis(row.attributeId)}
                    aria-label='Remove axis'
                  >
                    <Trash2 className='h-3.5 w-3.5' />
                  </Button>
                </div>
              </div>
              <div className='space-y-1'>
                <Label className='text-muted-foreground text-[11px] tracking-wide uppercase'>
                  Allowed values
                </Label>
                <div className='flex flex-wrap gap-2'>
                  {row.options.map((option) => {
                    const checked = row.allowedOptionIds.includes(option.id);
                    return (
                      <label
                        key={option.id}
                        className='border-border/70 bg-muted/20 inline-flex items-center gap-1.5 rounded-md border px-2 py-1 text-xs'
                      >
                        <input
                          type='checkbox'
                          checked={checked}
                          disabled={disabled}
                          onChange={() =>
                            toggleOption(row.attributeId, option.id)
                          }
                        />
                        <span>{option.label}</span>
                      </label>
                    );
                  })}
                </div>
              </div>
            </div>
          ))}
        </div>

        <div className='flex flex-wrap items-end gap-2'>
          <div className='grid gap-1'>
            <Label className='text-muted-foreground text-[11px] tracking-wide uppercase'>
              Attribute
            </Label>
            <Select
              value={addSelection}
              onValueChange={setAddSelection}
              disabled={disabled || availableCandidates.length === 0}
            >
              <SelectTrigger className='min-w-[220px]'>
                <SelectValue placeholder='Select attribute' />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={SELECT_EMPTY}>Select attribute</SelectItem>
                {availableCandidates.map((attribute) => (
                  <SelectItem
                    key={attribute.attributeId}
                    value={attribute.attributeId}
                  >
                    {attribute.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <Button
            type='button'
            variant='outline'
            size='sm'
            disabled={
              disabled ||
              addSelection === SELECT_EMPTY ||
              availableCandidates.length === 0
            }
            onClick={addAxis}
          >
            <Plus className='mr-2 h-4 w-4' />
            Add axis
          </Button>
          {availableCandidates.length === 0 && rows.length === 0 ? (
            <span className='text-muted-foreground text-xs'>
              The category schema has no option-backed attribute that allows
              axis use. Configure the category schema.
            </span>
          ) : null}
        </div>
      </CardContent>
    </Card>
  );
}
