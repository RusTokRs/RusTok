/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import { isEligibleAxis } from './schema-policy.ts';
import type {
  ProductEffectiveForm,
  SetVariantAxesInput,
  VariantAxisConfig,
  VariantAxisReassignmentInput,
  VariantAxisValueInput,
  VariantAxesConflictKind,
  VariantAxesPreview
} from './api/types';

export interface AxisRowDraft {
  attributeId: string;
  label: string;
  code: string;
  dictionaryLimited: boolean;
  allowedOptionIds: string[];
  options: Array<{ id: string; label: string; available: boolean }>;
}

/** Existing products are not silently seeded from changed schema defaults. */
export function seedReconfigurationAxes(
  axes: VariantAxisConfig[],
  form: ProductEffectiveForm | null
): AxisRowDraft[] {
  return refreshReconfigurationChoices(
    axes.map((axis) => ({
      attributeId: axis.attributeId,
      label: axis.name,
      code: axis.code,
      dictionaryLimited: false,
      allowedOptionIds: axis.allowedValues.map((value) => value.optionId),
      options: axis.allowedValues.map((value) => ({
        id: value.optionId,
        label: value.value || value.optionId,
        available: false
      }))
    })),
    form
  );
}

/** Update choice metadata only: no default reseeding, pruning or dirty-proposal rebasing. */
export function refreshReconfigurationChoices(
  rows: AxisRowDraft[],
  form: ProductEffectiveForm | null
): AxisRowDraft[] {
  return rows.map((row) => {
    const attribute = form?.attributes.find(
      (attribute) => attribute.attributeId === row.attributeId
    );
    const dictionaryLimited = (attribute?.options.length ?? 0) > 256;
    const options = (attribute?.options ?? [])
      .filter(
        (option) =>
          !dictionaryLimited || row.allowedOptionIds.includes(option.id)
      )
      .map((option) => ({
        id: option.id,
        label: option.label,
        available: !!attribute && isEligibleAxis(attribute)
      }));
    for (const id of row.allowedOptionIds) {
      if (!options.some((option) => option.id === id))
        options.push({
          id,
          label: row.options.find((option) => option.id === id)?.label ?? id,
          available: false
        });
    }
    return {
      ...row,
      label: attribute?.label ?? row.label,
      code: attribute?.code ?? row.code,
      dictionaryLimited,
      options
    };
  });
}

export function reconfigurationAxis(
  attributeId: string,
  form: ProductEffectiveForm | null
): AxisRowDraft | null {
  const attribute = form?.attributes.find(
    (item) => item.attributeId === attributeId
  );
  if (
    !attribute ||
    !isEligibleAxis(attribute) ||
    !attribute.options.length ||
    attribute.options.length > 256
  )
    return null;
  return {
    attributeId,
    code: attribute.code,
    label: attribute.label,
    dictionaryLimited: false,
    allowedOptionIds: attribute.options.map((option) => option.id),
    options: attribute.options.map((option) => ({
      id: option.id,
      label: option.label,
      available: true
    }))
  };
}

export function axesProposal(
  rows: AxisRowDraft[],
  reassignments: VariantAxisReassignmentInput[]
): SetVariantAxesInput {
  return {
    axes: rows.map((row, index) => ({
      attributeId: row.attributeId,
      position: index * 100,
      allowedOptionIds: [...row.allowedOptionIds]
    })),
    reassignments: structuredClone(reassignments)
  };
}

/** Editing never silently prunes a previous full tuple when the axis draft changes. The owner
 * reports unexpected assignments; the operator may explicitly reset that SKU's override. */
export function reassignAxisValue(
  reassignments: VariantAxisReassignmentInput[],
  source: VariantAxesPreview['variants'][number],
  attributeId: string,
  optionId: string
): VariantAxisReassignmentInput[] {
  const current = reassignments.find(
    (row) => row.variantId === source.variantId
  );
  const values = (current?.axisValues ?? source.proposedValues).filter(
    (value) => value.attributeId !== attributeId
  );
  if (optionId) values.push({ attributeId, optionId });
  const replacement = { variantId: source.variantId, axisValues: values };
  return [
    ...reassignments.filter((row) => row.variantId !== source.variantId),
    replacement
  ].sort((left, right) => left.variantId.localeCompare(right.variantId));
}

export function tupleSummary(
  values: VariantAxisValueInput[],
  rows: AxisRowDraft[],
  form: ProductEffectiveForm | null
): string {
  return values
    .map((value) => {
      const axis = rows.find((row) => row.attributeId === value.attributeId);
      const attribute = form?.attributes.find(
        (item) => item.attributeId === value.attributeId
      );
      const label = axis?.label ?? attribute?.label ?? value.attributeId;
      const option =
        axis?.options.find((option) => option.id === value.optionId)?.label ??
        attribute?.options.find((option) => option.id === value.optionId)
          ?.label ??
        value.optionId;
      return `${label}: ${option}`;
    })
    .join(' · ');
}

type AxesConflictMessageKey =
  | 'axes.conflict.missingAssignment'
  | 'axes.conflict.duplicateAssignment'
  | 'axes.conflict.unexpectedAssignment'
  | 'axes.conflict.valueNotAllowed'
  | 'axes.conflict.duplicateCombination'
  | 'axes.conflict.multipleDefaultVariants'
  | 'axes.conflict.unsupported';

export function axesConflictKey(
  kind: VariantAxesConflictKind
): AxesConflictMessageKey {
  switch (kind) {
    case 'missing_assignment':
      return 'axes.conflict.missingAssignment';
    case 'duplicate_assignment':
      return 'axes.conflict.duplicateAssignment';
    case 'unexpected_assignment':
      return 'axes.conflict.unexpectedAssignment';
    case 'value_not_allowed':
      return 'axes.conflict.valueNotAllowed';
    case 'duplicate_combination':
      return 'axes.conflict.duplicateCombination';
    case 'multiple_default_variants':
      return 'axes.conflict.multipleDefaultVariants';
    default:
      return 'axes.conflict.unsupported';
  }
}

const object = (value: unknown): value is Record<string, unknown> =>
  !!value && typeof value === 'object' && !Array.isArray(value);
const only = (value: Record<string, unknown>, keys: string[]) =>
  Object.keys(value).every((key) => keys.includes(key));
const uuid = (value: unknown): value is string =>
  typeof value === 'string' &&
  value !== '00000000-0000-0000-0000-000000000000' &&
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value);

/** Exact structural whitelist for durable browser recovery; no payload normalization on replay. */
export function isAxesApplicationInput(
  value: unknown
): value is SetVariantAxesInput {
  return (
    object(value) &&
    only(value, ['axes', 'expectedState', 'reassignments']) &&
    typeof value.expectedState === 'string' &&
    /^[0-9a-f]{64}$/.test(value.expectedState) &&
    Array.isArray(value.axes) &&
    value.axes.length <= 32 &&
    value.axes.every(
      (axis) =>
        object(axis) &&
        only(axis, ['attributeId', 'position', 'allowedOptionIds']) &&
        uuid(axis.attributeId) &&
        (axis.position === undefined ||
          axis.position === null ||
          (Number.isInteger(axis.position) &&
            (axis.position as number) >= 0 &&
            (axis.position as number) <= 2147483647)) &&
        Array.isArray(axis.allowedOptionIds) &&
        axis.allowedOptionIds.length >= 1 &&
        axis.allowedOptionIds.length <= 256 &&
        axis.allowedOptionIds.every(uuid)
    ) &&
    (value.reassignments === undefined ||
      (Array.isArray(value.reassignments) &&
        value.reassignments.length <= 256 &&
        value.reassignments.every(
          (row) =>
            object(row) &&
            only(row, ['variantId', 'axisValues']) &&
            uuid(row.variantId) &&
            Array.isArray(row.axisValues) &&
            row.axisValues.length <= 32 &&
            row.axisValues.every(
              (assignment) =>
                object(assignment) &&
                only(assignment, ['attributeId', 'optionId']) &&
                uuid(assignment.attributeId) &&
                uuid(assignment.optionId)
            )
        )))
  );
}

/** Shape/scope validation only, never a second combination planner or permission decision. */
export function validAxesPreview(
  productId: string,
  preview: VariantAxesPreview
): boolean {
  const tuple = (value: VariantAxisValueInput) =>
    uuid(value.attributeId) && uuid(value.optionId);
  return (
    preview.productId === productId &&
    Number.isInteger(preview.productRevision) &&
    preview.productRevision >= 0 &&
    Number.isInteger(preview.variantCount) &&
    preview.variantCount > 0 &&
    preview.variantCount <= 256 &&
    Array.isArray(preview.variants) &&
    preview.variants.length === preview.variantCount &&
    new Set(preview.variants.map((row) => row.variantId)).size ===
      preview.variantCount &&
    preview.variants.every(
      (row) =>
        uuid(row.variantId) &&
        Array.isArray(row.currentValues) &&
        Array.isArray(row.proposedValues) &&
        row.currentValues.length <= 32768 &&
        row.proposedValues.length <= 32768 &&
        row.currentValues.every(tuple) &&
        row.proposedValues.every(tuple)
    ) &&
    /^[0-9a-f]{64}$/.test(preview.stateToken) &&
    Array.isArray(preview.conflicts) &&
    preview.conflicts.length <= 16 &&
    preview.conflicts.every(
      (row) =>
        axesConflictKey(row.kind) !== 'axes.conflict.unsupported' &&
        Array.isArray(row.variantIds) &&
        row.variantIds.length <= 8 &&
        row.variantIds.every(uuid) &&
        (row.attributeId === null || uuid(row.attributeId)) &&
        (row.optionId === null || uuid(row.optionId))
    ) &&
    (!preview.canApply ||
      (!preview.conflicts.length && !preview.hasMoreConflicts))
  );
}

const axesDurableRefusals = new Set([
  'PRODUCT_VARIANT_AXES_PREVIEW_REQUIRED',
  'PRODUCT_VARIANT_AXES_STALE',
  'PRODUCT_VARIANT_AXES_CONFLICT',
  'PRODUCT_VARIANT_AXES_LIMIT_EXCEEDED',
  'PRODUCT_VALIDATION',
  'PRODUCT_DEPENDENCY_NOT_FOUND',
  'PRODUCT_CONFLICT',
  'PRODUCT_REVISION_CONFLICT',
  'PRODUCT_NOT_FOUND',
  'PRODUCT_VARIANT_NOT_FOUND',
  'PRODUCT_VARIANT_AXIS_POLICY_CONFLICT',
  'NO_VARIANTS'
]);
export function isAxesDurableRefusalCode(code: string): boolean {
  return axesDurableRefusals.has(code);
}

export function axesRefusalKey(
  code: string
): 'stale' | 'bounds' | 'conflicts' | 'failed' {
  switch (code) {
    case 'PRODUCT_VARIANT_AXES_PREVIEW_REQUIRED':
    case 'PRODUCT_VARIANT_AXES_STALE':
    case 'PRODUCT_REVISION_CONFLICT':
      return 'stale';
    case 'PRODUCT_VARIANT_AXES_LIMIT_EXCEEDED':
      return 'bounds';
    case 'PRODUCT_VARIANT_AXES_CONFLICT':
    case 'PRODUCT_CONFLICT':
    case 'NO_VARIANTS':
      return 'conflicts';
    default:
      return 'failed';
  }
}
