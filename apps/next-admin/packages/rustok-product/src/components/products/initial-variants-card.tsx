/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
'use client';

import * as React from 'react';
import {
  Card,
  CardHeader,
  CardTitle,
  CardContent,
  CardDescription
} from '@/shared/ui/shadcn/card';
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import { Checkbox } from '@/shared/ui/shadcn/checkbox';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import type {
  ProductEffectiveForm,
  VariantAxisDraftInput
} from '../../api/types';
import { CombinationPicker } from './combination-picker';
import { isEligibleAxis } from '../../schema-policy';
import {
  emptyInitialVariant,
  initialCombinationAxes,
  appendInitialCombination,
  MAX_INITIAL_VARIANTS,
  suggestedInitialAxes,
  setInitialAxis,
  setInitialAllowedValue,
  setInitialAssignment,
  type InitialVariantDraft
} from '../../variant-draft';

export function InitialVariantsCard({
  form,
  axes,
  variants,
  onAxesChange,
  onVariantsChange,
  disabled
}: {
  form: ProductEffectiveForm | null;
  axes: VariantAxisDraftInput[];
  variants: InitialVariantDraft[];
  onAxesChange: (axes: VariantAxisDraftInput[]) => void;
  onVariantsChange: (variants: InitialVariantDraft[]) => void;
  disabled: boolean;
}) {
  const [error, setError] = React.useState<string | null>(null);
  const eligible = form?.attributes.filter(isEligibleAxis) ?? [];
  const combinationAxes = React.useCallback(
    () => initialCombinationAxes(axes, form),
    [axes, form]
  );
  const combinationMembers = React.useMemo(
    () =>
      variants.map((variant, index) => ({
        id: `draft-${index + 1}`,
        sku: variant.sku,
        axisValues: variant.axisValues
      })),
    [variants]
  );
  const updateVariant = (index: number, patch: Partial<InitialVariantDraft>) =>
    onVariantsChange(
      variants.map((variant, row) =>
        row === index ? { ...variant, ...patch } : variant
      )
    );
  return (
    <Card>
      <CardHeader>
        <CardTitle>Initial axes and selected SKUs</CardTitle>
        <CardDescription>
          Only the rows you select are created, together with the product in one
          transaction. Category changes do not discard this draft. No Cartesian
          grid is generated.
        </CardDescription>
      </CardHeader>
      <CardContent className='space-y-4'>
        {error ? (
          <p role='alert' className='text-destructive'>
            {error}
          </p>
        ) : null}
        <Button
          type='button'
          variant='outline'
          disabled={disabled || !form || axes.length > 0}
          onClick={() => {
            try {
              onAxesChange(suggestedInitialAxes(form!));
              setError(null);
            } catch (failure) {
              setError(
                failure instanceof Error
                  ? failure.message
                  : 'Axis suggestions could not be loaded.'
              );
            }
          }}
        >
          Use required and suggested axes
        </Button>
        {eligible.map((attribute) => {
          const axis = axes.find(
            (candidate) => candidate.attributeId === attribute.attributeId
          );
          return (
            <fieldset
              key={attribute.attributeId}
              className='space-y-2 rounded border p-3'
              disabled={disabled}
            >
              <legend>
                {attribute.label} ({attribute.variantAxisPolicy})
              </legend>
              <label className='flex items-center gap-2'>
                <Checkbox
                  checked={!!axis}
                  disabled={
                    disabled ||
                    (!axis &&
                      (axes.length >= 32 ||
                        !attribute.options.length ||
                        attribute.options.length > 256))
                  }
                  onCheckedChange={(checked) => {
                    try {
                      onAxesChange(
                        setInitialAxis(
                          axes,
                          attribute.attributeId,
                          form!,
                          checked === true
                        )
                      );
                      setError(null);
                    } catch (failure) {
                      setError(
                        failure instanceof Error
                          ? failure.message
                          : 'Invalid axis selection.'
                      );
                    }
                  }}
                />{' '}
                Use as an axis
              </label>
              {axis ? (
                <div className='flex flex-wrap gap-3'>
                  {attribute.options.map((option) => (
                    <label
                      key={option.id}
                      className='flex items-center gap-1 text-xs'
                    >
                      <Checkbox
                        checked={
                          axis.allowedOptionIds?.includes(option.id) ?? false
                        }
                        disabled={disabled}
                        onCheckedChange={(checked) =>
                          onAxesChange(
                            setInitialAllowedValue(
                              axes,
                              axis.attributeId,
                              option.id,
                              checked === true
                            )
                          )
                        }
                      />
                      {option.label}
                    </label>
                  ))}
                </div>
              ) : null}
            </fieldset>
          );
        })}
        {axes
          .filter(
            (axis) =>
              !eligible.some(
                (attribute) => attribute.attributeId === axis.attributeId
              )
          )
          .map((axis) => (
            <p key={axis.attributeId} role='alert'>
              Axis {axis.attributeId} is no longer eligible in this category.{' '}
              <Button
                type='button'
                variant='outline'
                disabled={disabled}
                onClick={() =>
                  onAxesChange(axes.filter((candidate) => candidate !== axis))
                }
              >
                Remove draft axis
              </Button>
            </p>
          ))}
        {!eligible.length ? (
          <p className='text-muted-foreground text-sm'>
            No eligible axes in this category. A product without axes has one
            default SKU.
          </p>
        ) : null}
        <CombinationPicker
          getAxes={combinationAxes}
          members={combinationMembers}
          draft
          disabled={disabled}
          atCapacity={variants.length >= MAX_INITIAL_VARIANTS}
          onPick={(axisValues) => {
            if (disabled || variants.length >= MAX_INITIAL_VARIANTS) return;
            onVariantsChange(
              appendInitialCombination(axes, variants, form, axisValues)
            );
          }}
        />
        {variants.map((variant, index) => (
          <fieldset
            key={index}
            disabled={disabled}
            className='space-y-3 rounded border p-3'
          >
            <legend>Selected SKU {index + 1}</legend>
            <div className='grid gap-3 md:grid-cols-2'>
              {axes.map((axis) => {
                const attribute = form?.attributes.find(
                  (candidate) => candidate.attributeId === axis.attributeId
                );
                const selected = variant.axisValues.find(
                  (value) => value.attributeId === axis.attributeId
                )?.optionId;
                const valid =
                  axis.allowedOptionIds?.includes(selected ?? '') &&
                  attribute?.options.some((option) => option.id === selected);
                return (
                  <div key={axis.attributeId} className='space-y-1'>
                    <Label htmlFor={`draft-${index}-${axis.attributeId}`}>
                      {attribute?.label ?? axis.attributeId}
                    </Label>
                    <Select
                      value={valid ? selected : ''}
                      disabled={disabled}
                      onValueChange={(optionId) =>
                        updateVariant(index, {
                          axisValues: setInitialAssignment(
                            variant,
                            axis.attributeId,
                            optionId
                          )
                        })
                      }
                    >
                      <SelectTrigger id={`draft-${index}-${axis.attributeId}`}>
                        <SelectValue placeholder='Choose an allowed value' />
                      </SelectTrigger>
                      <SelectContent>
                        {attribute?.options
                          .filter((option) =>
                            axis.allowedOptionIds?.includes(option.id)
                          )
                          .map((option) => (
                            <SelectItem key={option.id} value={option.id}>
                              {option.label}
                            </SelectItem>
                          ))}
                      </SelectContent>
                    </Select>
                    {selected && !valid ? (
                      <p role='alert' className='text-xs'>
                        The retained value is no longer allowed. Select a
                        replacement.
                      </p>
                    ) : null}
                  </div>
                );
              })}
              {(
                [
                  'sku',
                  'barcode',
                  'priceAmount',
                  'currencyCode',
                  'compareAtAmount'
                ] as const
              ).map((field) => (
                <div key={field} className='space-y-1'>
                  <Label htmlFor={`initial-${index}-${field}`}>{field}</Label>
                  <Input
                    id={`initial-${index}-${field}`}
                    value={variant[field] ?? ''}
                    onChange={(event) =>
                      updateVariant(index, { [field]: event.target.value })
                    }
                    disabled={disabled}
                  />
                </div>
              ))}
              <div className='space-y-1'>
                <Label htmlFor={`initial-${index}-quantity`}>
                  Initial inventory
                </Label>
                <Input
                  id={`initial-${index}-quantity`}
                  type='number'
                  min='0'
                  step='1'
                  value={
                    Number.isNaN(variant.inventoryQuantity)
                      ? ''
                      : variant.inventoryQuantity
                  }
                  disabled={disabled}
                  onChange={(event) =>
                    updateVariant(index, {
                      inventoryQuantity:
                        event.target.value === ''
                          ? Number.NaN
                          : Number(event.target.value)
                    })
                  }
                />
              </div>
              <div className='space-y-1'>
                <Label htmlFor={`initial-${index}-policy`}>
                  Inventory policy
                </Label>
                <Select
                  value={variant.inventoryPolicy}
                  disabled={disabled}
                  onValueChange={(inventoryPolicy) =>
                    updateVariant(index, { inventoryPolicy })
                  }
                >
                  <SelectTrigger id={`initial-${index}-policy`}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value='deny'>Deny overselling</SelectItem>
                    <SelectItem value='continue'>Continue selling</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>
            {variant.axisValues.some(
              (value) =>
                !axes.some((axis) => axis.attributeId === value.attributeId)
            ) ? (
              <Button
                type='button'
                variant='outline'
                disabled={disabled}
                onClick={() =>
                  updateVariant(index, {
                    axisValues: variant.axisValues.filter((value) =>
                      axes.some(
                        (axis) => axis.attributeId === value.attributeId
                      )
                    )
                  })
                }
              >
                Remove assignments for removed draft axes
              </Button>
            ) : null}
            <Button
              type='button'
              variant='outline'
              disabled={disabled || variants.length === 1}
              onClick={() =>
                onVariantsChange(variants.filter((_, row) => row !== index))
              }
            >
              Remove draft SKU
            </Button>
          </fieldset>
        ))}
        <Button
          type='button'
          variant='outline'
          disabled={
            disabled ||
            axes.length === 0 ||
            variants.length >= MAX_INITIAL_VARIANTS
          }
          onClick={() => onVariantsChange([...variants, emptyInitialVariant()])}
        >
          Add selected SKU ({variants.length}/{MAX_INITIAL_VARIANTS})
        </Button>
      </CardContent>
    </Card>
  );
}
