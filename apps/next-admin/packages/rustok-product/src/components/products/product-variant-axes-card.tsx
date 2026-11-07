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
import { useTranslations } from '@rustok/next-fluent/client';
import { isEligibleAxis } from '../../schema-policy';
import {
  axesConflictKey,
  axesProposal,
  reassignAxisValue,
  reconfigurationAxis,
  seedReconfigurationAxes,
  refreshReconfigurationChoices,
  tupleSummary,
  validAxesPreview
} from '../../axis-reconfiguration';
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
  VariantAxisConfig,
  VariantAxisReassignmentInput,
  VariantAxesPreview
} from '../../api/types';

interface ProductVariantAxesCardProps {
  productId: string;
  locale: string;
  axes: VariantAxisConfig[];
  effectiveForm?: ProductEffectiveForm | null;
  onPreviewAxes?: (input: SetVariantAxesInput) => Promise<VariantAxesPreview>;
  /** The parent uses the same durable editorial journal as document/status operations. */
  onSaveAxes?: (input: SetVariantAxesInput) => Promise<void>;
  disabled?: boolean;
}

const SELECT_EMPTY = '__none__';
const PAGE_SIZE = 12;

export function ProductVariantAxesCard({
  productId,
  locale,
  axes,
  effectiveForm = null,
  onPreviewAxes,
  onSaveAxes,
  disabled = false
}: ProductVariantAxesCardProps) {
  const t = useTranslations('product');
  const uid = React.useId();
  const [rows, setRows] = React.useState(() =>
    seedReconfigurationAxes(axes, effectiveForm)
  );
  const [reassignments, setReassignments] = React.useState<
    VariantAxisReassignmentInput[]
  >([]);
  const [addSelection, setAddSelection] = React.useState(SELECT_EMPTY);
  const [isBusy, setIsBusy] = React.useState(false);
  const [confirmed, setConfirmed] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [notice, setNotice] = React.useState<string | null>(null);
  const [page, setPage] = React.useState(0);
  const [review, setReview] = React.useState<{
    signature: string;
    input: SetVariantAxesInput;
    preview: VariantAxesPreview;
  } | null>(null);
  React.useEffect(() => {
    setRows((rows) => refreshReconfigurationChoices(rows, effectiveForm));
    setConfirmed(false);
    setReview((old) => old && { ...old, signature: '' });
  }, [effectiveForm]);
  const proposal = React.useMemo(
    () => axesProposal(rows, reassignments),
    [rows, reassignments]
  );
  const signature = JSON.stringify({ productId, locale, proposal });
  const current = review?.signature === signature;
  const locked = disabled || isBusy;
  const candidates = (effectiveForm?.attributes ?? []).filter(
    (attribute) =>
      isEligibleAxis(attribute) &&
      attribute.options.length > 0 &&
      attribute.options.length <= 256 &&
      !rows.some((row) => row.attributeId === attribute.attributeId)
  );
  const pageCount = Math.max(
    1,
    Math.ceil((review?.preview.variants.length ?? 0) / PAGE_SIZE)
  );
  const currentPage = Math.min(page, pageCount - 1);
  const changed = () => {
    setConfirmed(false);
    setNotice(null);
    setError(null);
  };
  const move = (index: number, delta: number) => {
    changed();
    setRows((rows) => {
      const next = [...rows];
      const target = index + delta;
      if (target < 0 || target >= next.length) return rows;
      [next[index], next[target]] = [next[target], next[index]];
      return next;
    });
  };
  const preview = async () => {
    if (locked || !onPreviewAxes) return;
    const input = structuredClone(proposal);
    setIsBusy(true);
    setError(null);
    setNotice(null);
    setConfirmed(false);
    try {
      const result = await onPreviewAxes(input);
      if (!result || !validAxesPreview(productId, result)) {
        throw new Error(t('axes.failed'));
      }
      setReview({ signature, input, preview: result });
      setPage(0);
    } catch {
      setError(t('axes.failed'));
      setReview((old) => old && { ...old, signature: '' });
    } finally {
      setIsBusy(false);
    }
  };
  const apply = async () => {
    if (
      locked ||
      !onSaveAxes ||
      !review ||
      !current ||
      !review.preview.canApply ||
      !confirmed
    )
      return;
    setIsBusy(true);
    setError(null);
    setNotice(null);
    try {
      await onSaveAxes({
        ...structuredClone(review.input),
        expectedState: review.preview.stateToken
      });
      setNotice(t('axes.saved'));
    } catch {
      setError(t('axes.failed'));
      setReview((old) => old && { ...old, signature: '' });
    } finally {
      setIsBusy(false);
      setConfirmed(false);
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className='flex items-center gap-2 text-base'>
          {t('axes.title')} <Badge variant='secondary'>{rows.length}</Badge>
        </CardTitle>
        <CardDescription>
          {t('axes.description')} {t('axes.bounds')}
        </CardDescription>
        <p className='text-muted-foreground text-sm'>{t('axes.retained')}</p>
      </CardHeader>
      <CardContent className='space-y-4'>
        {!onPreviewAxes || !onSaveAxes ? (
          <p role='alert'>{t('axes.unavailable')}</p>
        ) : null}
        {error ? (
          <p role='alert' className='text-destructive text-sm'>
            {error}
          </p>
        ) : null}
        {notice ? (
          <p role='status' className='text-sm'>
            {notice}
          </p>
        ) : null}
        <fieldset disabled={locked} className='space-y-3'>
          {rows.length === 0 ? (
            <p className='text-muted-foreground text-sm'>{t('axes.empty')}</p>
          ) : null}
          {rows.map((row, index) => (
            <div
              key={row.attributeId}
              className='space-y-2 rounded-lg border p-3'
            >
              <div className='flex flex-wrap items-center justify-between gap-2'>
                <span className='text-sm font-medium'>
                  {row.label}{' '}
                  <span className='text-muted-foreground font-mono text-xs'>
                    {row.code}
                  </span>
                </span>
                <div className='flex gap-1'>
                  <Button
                    type='button'
                    variant='ghost'
                    size='icon'
                    aria-label={t('axes.up')}
                    disabled={index === 0}
                    onClick={() => move(index, -1)}
                  >
                    <ArrowUp className='h-4 w-4' />
                  </Button>
                  <Button
                    type='button'
                    variant='ghost'
                    size='icon'
                    aria-label={t('axes.down')}
                    disabled={index === rows.length - 1}
                    onClick={() => move(index, 1)}
                  >
                    <ArrowDown className='h-4 w-4' />
                  </Button>
                  <Button
                    type='button'
                    variant='ghost'
                    size='icon'
                    aria-label={t('axes.remove')}
                    onClick={() => {
                      changed();
                      setRows((rows) =>
                        rows.filter(
                          (item) => item.attributeId !== row.attributeId
                        )
                      );
                    }}
                  >
                    <Trash2 className='h-4 w-4' />
                  </Button>
                </div>
              </div>
              {row.dictionaryLimited ? (
                <p role='status' className='text-muted-foreground text-xs'>
                  {t('axes.dictionaryLimit')}
                </p>
              ) : null}
              <p className='text-muted-foreground text-xs'>
                {t('axes.allowed')}
              </p>
              <div className='flex flex-wrap gap-3'>
                {row.options.map((option) => (
                  <Label
                    key={option.id}
                    className='flex items-center gap-2 text-sm'
                  >
                    <input
                      type='checkbox'
                      checked={row.allowedOptionIds.includes(option.id)}
                      disabled={
                        !option.available &&
                        !row.allowedOptionIds.includes(option.id)
                      }
                      onChange={(event) => {
                        changed();
                        const selected = event.target.checked;
                        setRows((rows) =>
                          rows.map((item) =>
                            item.attributeId !== row.attributeId
                              ? item
                              : {
                                  ...item,
                                  allowedOptionIds: selected
                                    ? [...item.allowedOptionIds, option.id]
                                    : item.allowedOptionIds.filter(
                                        (id) => id !== option.id
                                      )
                                }
                          )
                        );
                      }}
                    />
                    {option.label}
                    {!option.available
                      ? ` (${t('axes.unavailableValue')})`
                      : ''}
                  </Label>
                ))}
              </div>
            </div>
          ))}
          <div className='flex flex-wrap gap-2'>
            <Select
              value={addSelection}
              onValueChange={setAddSelection}
              disabled={locked || rows.length >= 32 || candidates.length === 0}
            >
              <SelectTrigger
                className='min-w-48 flex-1'
                aria-label={t('axes.attribute')}
              >
                <SelectValue placeholder={t('axes.attribute')} />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={SELECT_EMPTY}>
                  {t('axes.attribute')}
                </SelectItem>
                {candidates.map((attribute) => (
                  <SelectItem
                    key={attribute.attributeId}
                    value={attribute.attributeId}
                  >
                    {attribute.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Button
              type='button'
              variant='outline'
              disabled={
                locked || rows.length >= 32 || addSelection === SELECT_EMPTY
              }
              onClick={() => {
                const row = reconfigurationAxis(addSelection, effectiveForm);
                if (
                  !row ||
                  rows.some((item) => item.attributeId === row.attributeId)
                )
                  return;
                changed();
                setRows((rows) => [...rows, row]);
                setAddSelection(SELECT_EMPTY);
              }}
            >
              <Plus className='mr-2 h-4 w-4' />
              {t('axes.add')}
            </Button>
          </div>
          {!candidates.length ? (
            <p className='text-muted-foreground text-xs'>
              {t('axes.noCandidates')}
            </p>
          ) : null}
        </fieldset>
        <Button
          type='button'
          variant='outline'
          disabled={locked || !onPreviewAxes}
          onClick={preview}
        >
          {isBusy ? t('axes.previewing') : t('axes.preview')}
        </Button>
        {review ? (
          <div className='space-y-3'>
            <p role='status' className='text-sm'>
              {!current
                ? t('axes.stale')
                : !review.preview.canApply
                  ? t('axes.conflicts')
                  : review.preview.hasChanges
                    ? t('axes.ready')
                    : t('axes.noChanges')}
            </p>
            <ul className='space-y-2 text-sm' aria-live='polite'>
              {review.preview.conflicts.map((conflict, index) => (
                <li
                  key={index}
                  className='border-destructive/40 rounded-md border p-2'
                >
                  {t(axesConflictKey(conflict.kind))}:{' '}
                  <span className='font-mono'>
                    {conflict.variantIds.join(', ')}
                  </span>
                  {conflict.attributeId ? (
                    <span className='block font-mono text-xs'>
                      {conflict.attributeId}
                      {conflict.optionId ? ` / ${conflict.optionId}` : ''}
                    </span>
                  ) : null}
                  {conflict.hasMoreVariants ? ` ${t('axes.moreVariants')}` : ''}
                </li>
              ))}
            </ul>
            {review.preview.hasMoreConflicts ? (
              <p role='alert'>{t('axes.moreConflicts')}</p>
            ) : null}
            <h4 className='text-sm font-semibold'>
              {t('axes.tuples')} ({review.preview.variantCount})
            </h4>
            <p className='text-muted-foreground text-xs'>
              {t('axes.tuplesHelp')}
            </p>
            <fieldset disabled={locked} className='space-y-3'>
              {review.preview.variants
                .slice(currentPage * PAGE_SIZE, (currentPage + 1) * PAGE_SIZE)
                .map((variant) => {
                  const override = reassignments.find(
                    (row) => row.variantId === variant.variantId
                  );
                  const values = override?.axisValues ?? variant.proposedValues;
                  return (
                    <div
                      key={variant.variantId}
                      className='space-y-2 rounded-lg border p-3'
                    >
                      <p className='text-sm font-medium'>
                        {variant.sku || variant.variantId}
                      </p>
                      <p className='text-muted-foreground font-mono text-xs'>
                        {variant.variantId}
                      </p>
                      <p className='text-xs'>
                        {t('axes.current')}:{' '}
                        {tupleSummary(
                          variant.currentValues,
                          rows,
                          effectiveForm
                        ) || t('axes.default')}
                      </p>
                      <p className='text-xs'>
                        {t('axes.proposed')}:{' '}
                        {tupleSummary(values, rows, effectiveForm) ||
                          (rows.length
                            ? t('axes.unassigned')
                            : t('axes.default'))}
                      </p>
                      {override?.axisValues.some(
                        (value) =>
                          !rows.some(
                            (row) => row.attributeId === value.attributeId
                          )
                      ) ? (
                        <p role='alert' className='text-destructive text-xs'>
                          {t('axes.unexpected')}
                        </p>
                      ) : null}
                      <div className='grid gap-3 sm:grid-cols-2'>
                        {rows.map((axis) => {
                          const selected =
                            values.find(
                              (value) => value.attributeId === axis.attributeId
                            )?.optionId ?? '';
                          const id = `${uid}-${variant.variantId}-${axis.attributeId}`;
                          return (
                            <div key={axis.attributeId} className='space-y-1'>
                              <Label htmlFor={id}>{axis.label}</Label>
                              <Select
                                value={selected || SELECT_EMPTY}
                                disabled={locked}
                                onValueChange={(value) => {
                                  changed();
                                  setReassignments((old) =>
                                    reassignAxisValue(
                                      old,
                                      variant,
                                      axis.attributeId,
                                      value === SELECT_EMPTY ? '' : value
                                    )
                                  );
                                }}
                              >
                                <SelectTrigger id={id}>
                                  <SelectValue />
                                </SelectTrigger>
                                <SelectContent>
                                  <SelectItem value={SELECT_EMPTY}>
                                    {t('axes.unassigned')}
                                  </SelectItem>
                                  {selected &&
                                  !axis.allowedOptionIds.includes(selected) ? (
                                    <SelectItem value={selected} disabled>
                                      {selected} ({t('axes.unavailableValue')})
                                    </SelectItem>
                                  ) : null}
                                  {axis.options
                                    .filter((option) =>
                                      axis.allowedOptionIds.includes(option.id)
                                    )
                                    .map((option) => (
                                      <SelectItem
                                        key={option.id}
                                        value={option.id}
                                        disabled={!option.available}
                                      >
                                        {option.label}
                                      </SelectItem>
                                    ))}
                                </SelectContent>
                              </Select>
                            </div>
                          );
                        })}
                      </div>
                      {override ? (
                        <Button
                          type='button'
                          size='sm'
                          variant='outline'
                          onClick={() => {
                            changed();
                            setReassignments((old) =>
                              old.filter(
                                (row) => row.variantId !== variant.variantId
                              )
                            );
                          }}
                        >
                          {t('axes.reset')}
                        </Button>
                      ) : null}
                    </div>
                  );
                })}
              <div className='flex items-center justify-between gap-2'>
                <Button
                  type='button'
                  size='sm'
                  variant='outline'
                  disabled={currentPage === 0}
                  onClick={() => setPage(currentPage - 1)}
                >
                  {t('axes.previous')}
                </Button>
                <span className='text-xs'>
                  {currentPage + 1} / {pageCount}
                </span>
                <Button
                  type='button'
                  size='sm'
                  variant='outline'
                  disabled={currentPage + 1 >= pageCount}
                  onClick={() => setPage(currentPage + 1)}
                >
                  {t('axes.next')}
                </Button>
              </div>
            </fieldset>
            <Label className='flex items-start gap-2 text-sm'>
              <input
                type='checkbox'
                checked={confirmed}
                disabled={
                  locked ||
                  !current ||
                  !review.preview.canApply ||
                  !review.preview.hasChanges
                }
                onChange={(event) => setConfirmed(event.target.checked)}
              />
              {t('axes.confirm')}
            </Label>
            <Button
              type='button'
              disabled={
                locked ||
                !current ||
                !review.preview.canApply ||
                !review.preview.hasChanges ||
                !confirmed ||
                !onSaveAxes
              }
              onClick={apply}
            >
              <Save className='mr-2 h-4 w-4' />
              {isBusy ? t('axes.applying') : t('axes.apply')}
            </Button>
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}
