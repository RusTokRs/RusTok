/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import type {
  ProductEffectiveFormAttribute,
  VariantAxisPolicy
} from './api/types';

export function isAxisAttribute(attribute: {
  valueType: string;
  scope: string;
}): boolean {
  return (
    attribute.valueType === 'select' &&
    (attribute.scope === 'variant' || attribute.scope === 'both')
  );
}

export function isEligibleAxis(
  attribute: ProductEffectiveFormAttribute
): boolean {
  return (
    !attribute.isDisabled &&
    isAxisAttribute(attribute) &&
    (attribute.variantAxisPolicy === 'allowed' ||
      attribute.variantAxisPolicy === 'required')
  );
}

/** Keep is omission, not a write of the database default on a repeated bind. */
export function buildAxisPolicyPatch(
  policy: string,
  defaultAxis: string
): {
  variantAxisPolicy?: VariantAxisPolicy;
  defaultVariantAxis?: boolean;
} {
  if (
    !['keep', 'forbidden', 'allowed', 'required'].includes(policy) ||
    !['keep', 'true', 'false'].includes(defaultAxis)
  ) {
    throw new Error('Invalid variant axis policy.');
  }
  if (policy === 'forbidden' && defaultAxis === 'true') {
    throw new Error('A forbidden attribute cannot be a default variant axis.');
  }
  return {
    ...(policy === 'keep'
      ? {}
      : { variantAxisPolicy: policy as VariantAxisPolicy }),
    ...(defaultAxis === 'keep'
      ? {}
      : { defaultVariantAxis: defaultAxis === 'true' })
  };
}

export function effectiveAxisPolicyLabel(
  attribute: ProductEffectiveFormAttribute
): string {
  const source =
    attribute.source === 'category_local'
      ? 'category override'
      : attribute.source.replaceAll('_', ' ');
  return `${attribute.variantAxisPolicy}; ${attribute.defaultVariantAxis ? 'default axis' : 'not a default axis'}; ${source}${attribute.isDisabled ? '; disabled' : ''}`;
}
