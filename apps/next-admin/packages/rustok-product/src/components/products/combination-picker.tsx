/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
'use client';

import { useMemo, useState } from 'react';
import { useTranslations } from '@rustok/next-fluent';
import { Button } from '@/shared/ui/shadcn/button';
import { Label } from '@/shared/ui/shadcn/label';
import { Alert, AlertDescription } from '@/shared/ui/shadcn/alert';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import type { VariantAxisValueInput } from '../../api/types';
import {
  CombinationError,
  combinationPage,
  chooseCombination,
  setCombinationFilter,
  type CombinationAxis,
  type CombinationMember,
  type CombinationProblem
} from '../../combinations';

/** Local preview only. The owner command remains the sole writer and uniqueness authority. */
export function CombinationPicker({
  getAxes,
  members,
  onPick,
  draft = false,
  disabled = false,
  atCapacity = false
}: {
  getAxes: () => CombinationAxis[];
  members: CombinationMember[];
  onPick: (values: VariantAxisValueInput[]) => void;
  draft?: boolean;
  disabled?: boolean;
  atCapacity?: boolean;
}) {
  const t = useTranslations('product.combinations');
  const [filters, setFilters] = useState<VariantAxisValueInput[]>([]);
  const [page, setPage] = useState(0);
  const [clickProblem, setClickProblem] = useState<CombinationProblem | null>(
    null
  );
  const { axes, result, sourceProblem } = useMemo(() => {
    let axes: CombinationAxis[] = [];
    try {
      axes = getAxes();
      return {
        axes,
        result: combinationPage(axes, filters, members, page, draft),
        sourceProblem: null
      };
    } catch (error) {
      return {
        axes,
        result: null,
        sourceProblem:
          error instanceof CombinationError ? error.code : ('failed' as const)
      };
    }
  }, [getAxes, filters, members, page, draft]);
  const problem = sourceProblem ?? clickProblem;
  const pick = (values: VariantAxisValueInput[]) => {
    if (disabled || atCapacity) return;
    try {
      const selected = chooseCombination(
        getAxes(),
        filters,
        members,
        values,
        draft
      );
      setClickProblem(null);
      onPick(selected);
    } catch (error) {
      setClickProblem(
        error instanceof CombinationError ? error.code : 'failed'
      );
    }
  };
  return (
    <fieldset disabled={disabled} className='space-y-3 rounded-md border p-3'>
      <legend className='px-1 text-sm font-medium'>{t('title')}</legend>
      <p className='text-muted-foreground text-xs'>
        {t(draft ? 'initialHelp' : 'savedHelp')}
      </p>
      <div className='grid gap-3 sm:grid-cols-2'>
        {axes.map((axis) => {
          const selected = filters.find(
            (value) => value.attributeId === axis.attributeId
          )?.optionId;
          const stale =
            selected && !axis.options.some((option) => option.id === selected);
          return (
            <div key={axis.attributeId} className='space-y-1'>
              <Label
                htmlFor={`combination-filter-${draft ? 'draft' : 'saved'}-${axis.attributeId}`}
              >
                {axis.label}
              </Label>
              <Select
                disabled={disabled}
                value={selected ?? '__any__'}
                onValueChange={(value) => {
                  if (disabled) return;
                  setFilters((current) =>
                    setCombinationFilter(
                      current,
                      axis.attributeId,
                      value === '__any__' ? '' : value
                    )
                  );
                  setPage(0);
                  setClickProblem(null);
                }}
              >
                <SelectTrigger
                  id={`combination-filter-${draft ? 'draft' : 'saved'}-${axis.attributeId}`}
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value='__any__'>{t('any')}</SelectItem>
                  {stale && (
                    <SelectItem value={selected} disabled>
                      {t('unavailable')} ({selected})
                    </SelectItem>
                  )}
                  {axis.options.map((option) => (
                    <SelectItem key={option.id} value={option.id}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          );
        })}
      </div>
      <Button
        type='button'
        variant='outline'
        onClick={() => {
          setFilters([]);
          setPage(0);
          setClickProblem(null);
        }}
      >
        {t('reset')}
      </Button>
      {problem && (
        <Alert variant='destructive'>
          <AlertDescription>{t(problem)}</AlertDescription>
        </Alert>
      )}
      {atCapacity && (
        <p role='status' className='text-muted-foreground text-xs'>
          {t('capacity')}
        </p>
      )}
      {result && (
        <>
          <p className='text-muted-foreground text-xs' aria-live='polite'>
            {t('count')} {result.count} · {t('page')} {result.page + 1} /{' '}
            {result.pages}
          </p>
          {!!result.incompleteDrafts && (
            <p role='status' className='text-xs'>
              {t('incomplete')} ({result.incompleteDrafts})
            </p>
          )}
          <ul className='space-y-2'>
            {result.rows.map((row) => (
              <li
                key={row.key}
                className='flex flex-wrap items-center justify-between gap-2 rounded border p-2 text-xs'
              >
                <span>
                  {row.labels.length ? row.labels.join(' · ') : t('default')}
                </span>
                {row.existing ? (
                  <span>
                    {t(draft ? 'selected' : 'existing')} ·{' '}
                    {row.existing.sku || row.existing.id}
                  </span>
                ) : (
                  <>
                    <span className='text-muted-foreground'>
                      {t('missing')}
                    </span>
                    <Button
                      type='button'
                      size='sm'
                      variant='outline'
                      disabled={disabled || atCapacity}
                      aria-label={`${t(draft ? 'add' : 'use')} — ${row.labels.length ? row.labels.join(' · ') : t('default')}`}
                      onClick={() => pick(row.axisValues)}
                    >
                      {t(draft ? 'add' : 'use')}
                    </Button>
                  </>
                )}
              </li>
            ))}
          </ul>
          <nav aria-label={t('navigation')} className='flex gap-2'>
            <Button
              type='button'
              variant='outline'
              size='sm'
              disabled={disabled || result.page === 0}
              onClick={() => setPage(Math.max(0, result.page - 1))}
            >
              {t('previous')}
            </Button>
            <Button
              type='button'
              variant='outline'
              size='sm'
              disabled={disabled || result.page + 1 >= result.pages}
              onClick={() => setPage(result.page + 1)}
            >
              {t('next')}
            </Button>
          </nav>
        </>
      )}
    </fieldset>
  );
}
