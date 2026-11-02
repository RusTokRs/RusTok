/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import type { VariantAxisConfig, VariantAxisValueInput } from './api/types';

/** Candidate browsing only. Neither these keys nor the missing rows are durable SKU identities. */
export const MAX_COMBINATION_CANDIDATES = 4096;
export const COMBINATION_PAGE_SIZE = 12;
export const MAX_COMBINATION_MEMBERS = 256;
export interface CombinationAxis {
  attributeId: string;
  label: string;
  options: { id: string; label: string }[];
}
export interface CombinationMember {
  id: string;
  sku?: string | null;
  axisValues: VariantAxisValueInput[];
}
export interface CombinationRow {
  key: string;
  axisValues: VariantAxisValueInput[];
  labels: string[];
  existing: CombinationMember | null;
}
export interface CombinationPage {
  count: number;
  page: number;
  pages: number;
  incompleteDrafts: number;
  rows: CombinationRow[];
}
export type CombinationProblem =
  'axes' | 'filters' | 'members' | 'limit' | 'taken' | 'capacity' | 'failed';
export class CombinationError extends Error {
  readonly code: CombinationProblem;
  constructor(code: CombinationProblem) {
    super(code);
    this.code = code;
  }
}

export function savedCombinationAxes(
  axes: VariantAxisConfig[]
): CombinationAxis[] {
  if (axes.length > 32 || axes.some((axis) => axis.allowedValues.length > 256))
    throw new CombinationError('axes');
  const result = axes.map((axis) => ({
    attributeId: axis.attributeId,
    label: axis.name,
    options: axis.allowedValues.map((value) => ({
      id: value.optionId,
      label: value.value
    }))
  }));
  validateAxes(result);
  return result;
}

/** Display the loaded assignments, not an availability decision; retain unknown UUIDs visibly. */
export function retainedCombinationLabels(
  axes: VariantAxisConfig[],
  values: (VariantAxisValueInput & {
    code?: string | null;
    label?: string | null;
  })[]
): string[] {
  return values.map((value) => {
    const axis = axes.find((axis) => axis.attributeId === value.attributeId);
    const option = axis?.allowedValues.find(
      (option) => option.optionId === value.optionId
    );
    return `${axis?.name ?? value.code ?? value.attributeId}: ${option?.value ?? value.label ?? value.optionId}`;
  });
}

type AxisIndex = Map<string, Set<string>>;
function validateAxes(axes: CombinationAxis[]): AxisIndex {
  const seen: AxisIndex = new Map();
  if (!Array.isArray(axes) || axes.length > 32)
    throw new CombinationError('axes');
  for (const axis of axes) {
    if (
      !axis ||
      typeof axis.attributeId !== 'string' ||
      !axis.attributeId ||
      seen.has(axis.attributeId) ||
      !Array.isArray(axis.options) ||
      !axis.options.length ||
      axis.options.length > 256 ||
      axis.options.some(
        (option) => !option || typeof option.id !== 'string' || !option.id
      ) ||
      new Set(axis.options.map((option) => option.id)).size !==
        axis.options.length
    )
      throw new CombinationError('axes');
    seen.set(
      axis.attributeId,
      new Set(axis.options.map((option) => option.id))
    );
  }
  return seen;
}

/** Exact, order-independent local tuple key; no pruning or defaults for incomplete assignments. */
function tupleKey(
  axes: AxisIndex,
  values: VariantAxisValueInput[]
): string | null {
  if (!Array.isArray(values) || values.length !== axes.size) return null;
  const seen = new Set<string>();
  for (const value of values) {
    if (
      !value ||
      seen.has(value.attributeId) ||
      !axes.get(value.attributeId)?.has(value.optionId)
    )
      return null;
    seen.add(value.attributeId);
  }
  return JSON.stringify(
    values
      .map((value) => [value.attributeId, value.optionId])
      .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
  );
}

function prepare(
  axes: CombinationAxis[],
  filters: VariantAxisValueInput[],
  members: CombinationMember[],
  draft: boolean
) {
  const allowed = validateAxes(axes);
  if (!Array.isArray(filters) || filters.length > 32)
    throw new CombinationError('filters');
  if (!Array.isArray(members) || members.length > MAX_COMBINATION_MEMBERS)
    throw new CombinationError('members');
  if (!draft && !axes.length && members.length !== 1)
    throw new CombinationError('members');
  const fixed = new Map<string, string>();
  for (const value of filters) {
    if (
      !value ||
      fixed.has(value.attributeId) ||
      !allowed.get(value.attributeId)?.has(value.optionId)
    )
      throw new CombinationError('filters');
    fixed.set(value.attributeId, value.optionId);
  }
  const index = new Map<string, CombinationMember>();
  const ids = new Set<string>();
  let incompleteDrafts = 0;
  for (const member of members) {
    if (
      !member ||
      typeof member.id !== 'string' ||
      !member.id ||
      ids.has(member.id) ||
      (member.sku != null && typeof member.sku !== 'string')
    )
      throw new CombinationError('members');
    ids.add(member.id);
    const key = tupleKey(allowed, member.axisValues);
    if (key === null) {
      if (!draft) throw new CombinationError('members');
      incompleteDrafts++;
      continue;
    }
    if (index.has(key)) throw new CombinationError('members');
    index.set(key, member);
  }
  const dimensions = axes.map((axis) => ({
    ...axis,
    options: fixed.has(axis.attributeId)
      ? axis.options.filter(
          (option) => option.id === fixed.get(axis.attributeId)
        )
      : axis.options
  }));
  let count = 1;
  for (const axis of dimensions) {
    if (count > Math.floor(MAX_COMBINATION_CANDIDATES / axis.options.length))
      throw new CombinationError('limit');
    count *= axis.options.length;
  }
  for (const [attributeId, optionId] of fixed)
    allowed.set(attributeId, new Set([optionId]));
  return { dimensions, count, index, incompleteDrafts, allowed };
}

export function combinationPage(
  axes: CombinationAxis[],
  filters: VariantAxisValueInput[],
  members: CombinationMember[],
  requestedPage: number,
  draft = false
): CombinationPage {
  if (!Number.isSafeInteger(requestedPage) || requestedPage < 0)
    throw new CombinationError('filters');
  const { dimensions, count, index, incompleteDrafts, allowed } = prepare(
    axes,
    filters,
    members,
    draft
  );
  const pages = Math.ceil(count / COMBINATION_PAGE_SIZE),
    page = Math.min(requestedPage, pages - 1);
  const start = page * COMBINATION_PAGE_SIZE;
  const rows: CombinationRow[] = [];
  // Mixed-radix random access: allocate a page, not the Cartesian matrix, even at the bound.
  for (
    let position = start;
    position < Math.min(count, start + COMBINATION_PAGE_SIZE);
    position++
  ) {
    let cursor = position;
    const axisValues: VariantAxisValueInput[] = [],
      labels: string[] = [];
    for (let i = dimensions.length - 1; i >= 0; i--) {
      const axis = dimensions[i],
        option = axis.options[cursor % axis.options.length];
      cursor = Math.floor(cursor / axis.options.length);
      axisValues.unshift({
        attributeId: axis.attributeId,
        optionId: option.id
      });
      labels.unshift(`${axis.label}: ${option.label}`);
    }
    const key = tupleKey(allowed, axisValues);
    if (key === null) throw new CombinationError('axes');
    rows.push({ key, axisValues, labels, existing: index.get(key) ?? null });
  }
  return { count, pages, page, incompleteDrafts, rows };
}

/** Recheck current source/filter/membership on click; stale DOM is never an availability proof. */
export function chooseCombination(
  axes: CombinationAxis[],
  filters: VariantAxisValueInput[],
  members: CombinationMember[],
  values: VariantAxisValueInput[],
  draft = false
): VariantAxisValueInput[] {
  const { allowed, index } = prepare(axes, filters, members, draft);
  const key = tupleKey(allowed, values);
  if (key === null) throw new CombinationError('filters');
  if (index.has(key)) throw new CombinationError('taken');
  return values.map((value) => ({
    attributeId: value.attributeId,
    optionId: value.optionId
  }));
}

export function setCombinationFilter(
  filters: VariantAxisValueInput[],
  attributeId: string,
  optionId: string
): VariantAxisValueInput[] {
  return [
    ...filters.filter((value) => value.attributeId !== attributeId),
    ...(optionId ? [{ attributeId, optionId }] : [])
  ];
}
