/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

import type {
  ProductAttributeValueBatch,
  ProductAttributeValueItem,
  ProductAttributeValuePatch
} from './api/types';

export type ProductAttributeValueDraftState = {
  activeLocale: string;
  valuesByLocale: Record<string, Record<string, ProductAttributeValuePatch>>;
  nonLocalizedValues: Record<string, ProductAttributeValuePatch>;
  dirtyLocalizedAttributeIdsByLocale: Record<string, ReadonlySet<string>>;
  dirtyNonLocalizedAttributeIds: ReadonlySet<string>;
};

const compareStrings = (left: string, right: string): number =>
  left < right ? -1 : left > right ? 1 : 0;

export function toProductAttributeValueMap(
  values: readonly ProductAttributeValueItem[]
): Record<string, ProductAttributeValuePatch> {
  const result: Record<string, ProductAttributeValuePatch> = {};
  for (const value of values) {
    result[value.attributeId] = {
      attributeId: value.attributeId,
      kind: value.kind,
      text: value.text,
      integer: value.integer,
      decimal: value.decimal,
      boolean: value.boolean,
      date: value.date,
      datetime: value.datetime,
      optionId: value.optionId,
      optionIds: value.optionIds,
      json: value.json
    };
  }
  return result;
}

/**
 * Build one deterministic batch per locale. Localized drafts stay with their
 * locale; shared nonlocalized edits are emitted exactly once in the active
 * locale's batch so the owner never receives duplicate global patches.
 */
export function buildProductAttributeValueBatches(
  state: ProductAttributeValueDraftState
): ProductAttributeValueBatch[] {
  const batches = new Map<string, ProductAttributeValuePatch[]>();

  for (const locale of Object.keys(
    state.dirtyLocalizedAttributeIdsByLocale
  ).sort(compareStrings)) {
    const ids = Array.from(
      state.dirtyLocalizedAttributeIdsByLocale[locale] ?? []
    ).sort(compareStrings);
    const patches = ids
      .map((id) => state.valuesByLocale[locale]?.[id])
      .filter(
        (patch): patch is ProductAttributeValuePatch => patch !== undefined
      );
    if (patches.length > 0) batches.set(locale, patches);
  }

  const sharedPatches = Array.from(
    state.dirtyNonLocalizedAttributeIds
  )
    .sort(compareStrings)
    .map((id) => state.nonLocalizedValues[id])
    .filter(
      (patch): patch is ProductAttributeValuePatch => patch !== undefined
    );

  if (sharedPatches.length > 0) {
    const combined = [
      ...(batches.get(state.activeLocale) ?? []),
      ...sharedPatches
    ].sort((left, right) =>
      compareStrings(left.attributeId, right.attributeId)
    );
    batches.set(state.activeLocale, combined);
  }

  return Array.from(batches.entries())
    .sort(([left], [right]) => compareStrings(left, right))
    .map(([locale, patches]) => ({ locale, patches }));
}
