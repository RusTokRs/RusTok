/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import type {
  ProductEffectiveForm,
  VariantAxisDraftInput,
  VariantAxisValueInput
} from './api/types';
import type { CombinationAxis } from './combinations';
import { chooseCombination, CombinationError } from './combinations.ts';
import { isEligibleAxis } from './schema-policy';
import { parsePriceAmount } from './money';

/** A bounded explicit selection, never an automatically materialized Cartesian product. */
export const MAX_INITIAL_VARIANTS = 128;
export interface InitialVariantScalars {
  sku: string;
  barcode: string;
  priceAmount: string;
  currencyCode: string;
  compareAtAmount?: string | null;
  inventoryQuantity: number;
  inventoryPolicy: string;
}
export interface InitialVariantDraft extends InitialVariantScalars {
  axisValues: VariantAxisValueInput[];
}
export const emptyInitialVariant = (): InitialVariantDraft => ({
  sku: '',
  barcode: '',
  priceAmount: '',
  currencyCode: 'USD',
  compareAtAmount: null,
  inventoryQuantity: 0,
  inventoryPolicy: 'deny',
  axisValues: []
});

export function suggestedInitialAxes(
  form: ProductEffectiveForm
): VariantAxisDraftInput[] {
  const attributes = form.attributes.filter(
    (attribute) =>
      isEligibleAxis(attribute) &&
      (attribute.defaultVariantAxis ||
        attribute.variantAxisPolicy === 'required')
  );
  if (attributes.length > 32)
    throw new Error('At most 32 variant axes are supported.');
  return attributes.map((attribute, position) => {
    if (attribute.options.length < 1 || attribute.options.length > 256) {
      throw new Error(
        `${attribute.label} must have 1–256 allowed options. Configure its dictionary first.`
      );
    }
    return {
      attributeId: attribute.attributeId,
      position,
      allowedOptionIds: attribute.options.map((option) => option.id)
    };
  });
}

export function validateAxisValues(
  axes: VariantAxisDraftInput[],
  values: VariantAxisValueInput[]
): string {
  if (values.length !== axes.length)
    throw new Error('Select exactly one value for every configured axis.');
  const selected = new Map<string, string>();
  for (const value of values) {
    const axis = axes.find(
      (candidate) => candidate.attributeId === value.attributeId
    );
    if (
      !axis ||
      selected.has(value.attributeId) ||
      !axis.allowedOptionIds?.includes(value.optionId)
    ) {
      throw new Error(
        'A variant contains a duplicate axis or a value outside its allowed set.'
      );
    }
    selected.set(value.attributeId, value.optionId);
  }
  // Local identity is for draft validation only. The Product owner computes the durable identity.
  return JSON.stringify(
    [...selected].sort(([left], [right]) => left.localeCompare(right))
  );
}

export function validateInitialAxes(
  axes: VariantAxisDraftInput[],
  form: ProductEffectiveForm | null
): void {
  if (
    axes.length > 32 ||
    new Set(axes.map((axis) => axis.attributeId)).size !== axes.length
  ) {
    throw new Error('Configure at most 32 distinct variant axes.');
  }
  for (const axis of axes) {
    const attribute = form?.attributes.find(
      (candidate) => candidate.attributeId === axis.attributeId
    );
    if (!attribute || !isEligibleAxis(attribute))
      throw new Error(
        'An axis is not allowed by the selected category. Review the draft; it has not been reset.'
      );
    const options = axis.allowedOptionIds ?? [];
    if (
      options.length < 1 ||
      options.length > 256 ||
      new Set(options).size !== options.length ||
      options.some(
        (id) => !attribute.options.some((option) => option.id === id)
      )
    ) {
      throw new Error(
        'Every axis must contain 1–256 distinct active dictionary values.'
      );
    }
  }
  for (const attribute of form?.attributes ?? []) {
    if (
      !attribute.isDisabled &&
      attribute.variantAxisPolicy === 'required' &&
      !axes.some((axis) => axis.attributeId === attribute.attributeId)
    ) {
      throw new Error(`Required axis ${attribute.label} is missing.`);
    }
  }
}

export function initialCombinationAxes(
  axes: VariantAxisDraftInput[],
  form: ProductEffectiveForm | null
): CombinationAxis[] {
  try {
    validateInitialAxes(axes, form);
  } catch {
    throw new CombinationError('axes');
  }
  return axes.map((axis) => {
    const attribute = form?.attributes.find(
      (attribute) => attribute.attributeId === axis.attributeId
    );
    if (!attribute) throw new CombinationError('axes');
    return {
      attributeId: axis.attributeId,
      label: attribute.label,
      options: (axis.allowedOptionIds ?? []).map((id) => {
        const option = attribute.options.find((option) => option.id === id);
        if (!option) throw new CombinationError('axes');
        return { id, label: option.label };
      })
    };
  });
}

/** Append exactly one explicit local row; final save still validates every retained draft. */
export function appendInitialCombination(
  axes: VariantAxisDraftInput[],
  variants: InitialVariantDraft[],
  form: ProductEffectiveForm | null,
  values: VariantAxisValueInput[]
): InitialVariantDraft[] {
  if (variants.length >= MAX_INITIAL_VARIANTS)
    throw new CombinationError('capacity');
  const selected = chooseCombination(
    initialCombinationAxes(axes, form),
    // A known full tuple is a single candidate: do not apply the unfiltered browsing bound.
    values,
    variants.map((variant, index) => ({
      id: `draft-${index + 1}`,
      sku: variant.sku,
      axisValues: variant.axisValues
    })),
    values,
    true
  );
  return [...variants, { ...emptyInitialVariant(), axisValues: selected }];
}

export function normalizeInitialCatalog(
  axes: VariantAxisDraftInput[],
  variants: InitialVariantDraft[],
  form: ProductEffectiveForm | null
): InitialVariantDraft[] {
  validateInitialAxes(axes, form);
  if (
    variants.length < 1 ||
    variants.length > MAX_INITIAL_VARIANTS ||
    (!axes.length && variants.length !== 1)
  ) {
    throw new Error(
      `Select 1–${MAX_INITIAL_VARIANTS} initial SKUs; a product without axes has exactly one default SKU.`
    );
  }
  const identities = new Set<string>();
  const skus = new Set<string>();
  return variants.map((variant) => {
    const identity = validateAxisValues(axes, variant.axisValues);
    if (identities.has(identity))
      throw new Error('Two draft SKUs have the same axis combination.');
    identities.add(identity);
    const sku = variant.sku.trim();
    if (sku && skus.has(sku))
      throw new Error('Draft SKU codes must be unique.');
    if (sku) skus.add(sku);
    if (
      !Number.isInteger(variant.inventoryQuantity) ||
      variant.inventoryQuantity < 0 ||
      variant.inventoryQuantity > 2147483647
    ) {
      throw new Error(
        'Initial inventory must be a non-negative 32-bit integer.'
      );
    }
    if (
      variant.sku.length > 100 ||
      variant.barcode.length > 100 ||
      (!!variant.priceAmount.trim() &&
        !/^[A-Za-z]{3}$/.test(variant.currencyCode)) ||
      !['deny', 'continue'].includes(variant.inventoryPolicy)
    ) {
      throw new Error(
        'Check SKU/barcode length, currency and inventory policy.'
      );
    }
    if (!variant.priceAmount.trim() && variant.compareAtAmount?.trim())
      throw new Error('A compare-at price requires an explicit base price.');
    return {
      ...variant,
      priceAmount: variant.priceAmount.trim()
        ? parsePriceAmount(variant.priceAmount)
        : '',
      compareAtAmount: variant.compareAtAmount?.trim()
        ? parsePriceAmount(variant.compareAtAmount)
        : null,
      axisValues: variant.axisValues.map((value) => ({ ...value }))
    };
  });
}

export function initialVariantInput(variant: InitialVariantDraft) {
  if (!variant.priceAmount.trim() && variant.compareAtAmount?.trim())
    throw new Error('A compare-at price requires an explicit base price.');
  return {
    sku: variant.sku || null,
    barcode: variant.barcode || null,
    shippingProfileSlug: null,
    axisValues: variant.axisValues,
    prices: variant.priceAmount.trim()
      ? [
          {
            currencyCode: variant.currencyCode,
            amount: variant.priceAmount,
            compareAtAmount: variant.compareAtAmount ?? null
          }
        ]
      : [],
    inventoryQuantity: variant.inventoryQuantity,
    inventoryPolicy: variant.inventoryPolicy
  };
}

export function orderedInitialAxes(
  axes: VariantAxisDraftInput[]
): VariantAxisDraftInput[] {
  return axes.map((axis, position) => ({ ...axis, position }));
}

export function setInitialAxis(
  axes: VariantAxisDraftInput[],
  attributeId: string,
  form: ProductEffectiveForm,
  selected: boolean
): VariantAxisDraftInput[] {
  if (!selected)
    return orderedInitialAxes(
      axes.filter((axis) => axis.attributeId !== attributeId)
    );
  const attribute = form.attributes.find(
    (attribute) =>
      attribute.attributeId === attributeId && isEligibleAxis(attribute)
  );
  if (
    !attribute ||
    axes.length >= 32 ||
    !attribute.options.length ||
    attribute.options.length > 256
  )
    throw new Error(
      'Select an eligible axis with 1–256 values (at most 32 axes).'
    );
  if (axes.some((axis) => axis.attributeId === attributeId)) return axes;
  return orderedInitialAxes([
    ...axes,
    {
      attributeId,
      allowedOptionIds: attribute.options.map((option) => option.id)
    }
  ]);
}

export function setInitialAllowedValue(
  axes: VariantAxisDraftInput[],
  attributeId: string,
  optionId: string,
  selected: boolean
): VariantAxisDraftInput[] {
  return axes.map((axis) =>
    axis.attributeId !== attributeId
      ? axis
      : {
          ...axis,
          allowedOptionIds: selected
            ? [...new Set([...(axis.allowedOptionIds ?? []), optionId])]
            : axis.allowedOptionIds?.filter((id) => id !== optionId)
        }
  );
}

export function setInitialAssignment(
  variant: InitialVariantDraft,
  attributeId: string,
  optionId: string
): InitialVariantDraft['axisValues'] {
  return [
    ...variant.axisValues.filter((value) => value.attributeId !== attributeId),
    { attributeId, optionId }
  ];
}
