/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

/**
 * Framework-free facet contract shared by every RusTok table surface.
 *
 * This module is the TypeScript twin of `crates/ui/rustok-grid::facet`: it carries *what* an owner
 * counted (a facet with its buckets and its domain) and the `key=value` selection vocabulary the
 * panel mutates. It deliberately knows nothing about React, Next, routing or the HTTP layer, so the
 * Next storefront, the Next admin tables and any future table render the same numbers and speak
 * the same selection language as the Leptos grids do through `rustok-grid`.
 */

/** Separator between the key and the value of one selection entry (`code=value`). */
export const SELECTION_SEPARATOR = "=";

/** Separator between selection entries inside one route value (`color=red;size=m`). */
export const SELECTION_LIST_SEPARATOR = ";";

/** Maximum number of facets one grid panel renders; mirrors `rustok_grid::MAX_GRID_FACETS`. */
export const MAX_GRID_FACETS = 8;

/** Maximum number of bucket values one enumerable facet carries; mirrors `rustok-grid`. */
export const MAX_GRID_FACET_VALUES = 20;

/**
 * What a facet's value domain is, mirroring `rustok_grid::FacetDomain`.
 *
 * `dictionary` is an enumerable option list (`multi` when several values may be selected at once),
 * `boolean` is the two-state domain, and `open` is an unbounded domain (text, numeric, date) that
 * has no bucket list at all and keeps the free-form filter input instead.
 */
export type FacetDomain =
  | { kind: "dictionary"; multi: boolean }
  | { kind: "boolean" }
  | { kind: "open" };

/** One counted bucket of a facet. */
export type FacetBucket = {
  value: string;
  label: string;
  count: number;
};

/** A facet an owner resolved for the current filter set. */
export type FacetSource = {
  code: string;
  label: string;
  domain: FacetDomain;
  /** True when the domain enumerates values; `open` domains never do. */
  isEnumerable: boolean;
  /** True when the bucket list was cut at {@link MAX_GRID_FACET_VALUES}. */
  isTruncated: boolean;
  /** Products matching every other active facet that carry a value for this facet. */
  total: number;
  values: FacetBucket[];
};

export function dictionaryDomain(multi: boolean): FacetDomain {
  return { kind: "dictionary", multi };
}

export function booleanDomain(): FacetDomain {
  return { kind: "boolean" };
}

export function openDomain(): FacetDomain {
  return { kind: "open" };
}

export function facetDomainIsEnumerable(domain: FacetDomain): boolean {
  return domain.kind !== "open";
}

/**
 * Builds an enumerable facet from counted buckets.
 *
 * The bucket list is cut at {@link MAX_GRID_FACET_VALUES} and the cut is reported through
 * `isTruncated` instead of being silent, exactly like `GridFacet::from_buckets` does for the Leptos
 * grids: a panel may show fewer values than the owner counted, but it must say so. An open domain
 * keeps an empty bucket list, again like its Rust twin, because an unbounded attribute has nothing
 * to enumerate.
 */
export function facetFromBuckets(input: {
  code: string;
  label: string;
  domain: FacetDomain;
  total: number;
  values: readonly FacetBucket[];
}): FacetSource {
  const isEnumerable = facetDomainIsEnumerable(input.domain);
  const values: FacetBucket[] = [];
  let isTruncated = false;
  // Only an enumerable domain has a bucket list: an open facet keeps none, exactly like
  // `GridFacet::from_buckets`, so a caller may hand over whatever the owner sent without the panel
  // inventing buckets for a free-text, numeric or date attribute.
  if (isEnumerable) {
    for (const bucket of input.values ?? []) {
      if (values.length === MAX_GRID_FACET_VALUES) {
        isTruncated = true;
        break;
      }
      values.push(bucket);
    }
  }
  return {
    code: input.code,
    label: input.label,
    domain: input.domain,
    isEnumerable,
    isTruncated,
    total: input.total,
    values,
  };
}

/** Builds an unbounded facet: the owner reported a total, the domain has no buckets. */
export function openFacet(input: {
  code: string;
  label: string;
  total: number;
}): FacetSource {
  return {
    code: input.code,
    label: input.label,
    domain: openDomain(),
    isEnumerable: false,
    isTruncated: false,
    total: input.total,
    values: [],
  };
}

/** The counted buckets as selection options, mirroring `GridFacet::filter_options`. */
export function facetFilterOptions(
  facet: FacetSource,
): Array<{ value: string; label: string; count: number }> {
  return facet.values.map((bucket) => ({
    value: bucket.value,
    label: bucket.label,
    count: bucket.count,
  }));
}

/** The `key=value` selection entry a bucket toggles. */
export function selectionEntry(key: string, value: string): string {
  return `${key}${SELECTION_SEPARATOR}${value}`;
}

/** The key part of one selection entry, or `null` when the entry addresses nothing. */
export function selectionKey(entry: string): string | null {
  const separator = entry.indexOf(SELECTION_SEPARATOR);
  if (separator < 0) return null;
  const key = entry.slice(0, separator).trim();
  return key.length > 0 ? key : null;
}

/** Splits one selection entry into its parts; both must be non-empty after trimming. */
export function splitSelection(
  entry: string,
): { key: string; value: string } | null {
  const separator = entry.indexOf(SELECTION_SEPARATOR);
  if (separator < 0) return null;
  const key = entry.slice(0, separator).trim();
  const value = entry.slice(separator + 1).trim();
  if (key.length === 0 || value.length === 0) return null;
  return { key, value };
}

function sameKey(candidate: string, wanted: string): boolean {
  return candidate.toLowerCase() === wanted.toLowerCase();
}

/** Reads the list-joined route value back into selection entries. */
export function parseSelection(value?: string | null): string[] {
  const normalized = (value ?? "").trim();
  if (normalized.length === 0) return [];
  return normalized
    .split(SELECTION_LIST_SEPARATOR)
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);
}

/** Serializes selection entries back into the single route value. */
export function serializeSelection(selected: readonly string[]): string {
  return selected.join(SELECTION_LIST_SEPARATOR);
}

/**
 * True when `key=value` is part of the selection.
 *
 * Keys compare case-insensitively (attribute codes arrive from several layers) while values must
 * match exactly, because a value is an option id: `SIZE=m` and `size=m` address the same facet,
 * `size=M` does not.
 */
export function isSelectionSelected(
  selected: readonly string[],
  key: string,
  value: string,
): boolean {
  const wantedKey = key.trim();
  const wantedValue = value.trim();
  return selected.some((entry) => {
    const parsed = splitSelection(entry);
    return (
      parsed !== null &&
      sameKey(parsed.key, wantedKey) &&
      parsed.value === wantedValue
    );
  });
}

/** True when any value of `key` is selected; keys compare case-insensitively. */
export function hasSelectionForKey(
  selected: readonly string[],
  key: string,
): boolean {
  const wantedKey = key.trim();
  if (wantedKey.length === 0) return false;
  return selected.some((entry) => {
    const parsed = splitSelection(entry);
    return parsed !== null && sameKey(parsed.key, wantedKey);
  });
}

/** The selected entries that belong to `key`, in their original order. */
export function selectionForKey(
  selected: readonly string[],
  key: string,
): string[] {
  const wantedKey = key.trim();
  if (wantedKey.length === 0) return [];
  return selected.filter((entry) => {
    const parsed = splitSelection(entry);
    return parsed !== null && sameKey(parsed.key, wantedKey);
  });
}

/** The selection an owner must keep while counting `ownKey` (drill-down counts). */
export function selectionExcept(
  selected: readonly string[],
  ownKey: string,
): string[] {
  const wantedKey = ownKey.trim();
  if (wantedKey.length === 0) return [...selected];
  return selected.filter((entry) => {
    const parsed = splitSelection(entry);
    return parsed === null || !sameKey(parsed.key, wantedKey);
  });
}

/**
 * Flips `key=value`: an active entry is dropped, an unknown one is appended, and every surviving
 * entry keeps its position, so repeated toggles produce stable links.
 */
export function selectionAfterToggle(
  selected: readonly string[],
  key: string,
  value: string,
): string[] {
  const wantedKey = key.trim();
  const wantedValue = value.trim();
  if (wantedKey.length === 0 || wantedValue.length === 0) return [...selected];
  const toggled: string[] = [];
  let removed = false;
  for (const entry of selected) {
    const parsed = splitSelection(entry);
    if (
      !removed &&
      parsed !== null &&
      sameKey(parsed.key, wantedKey) &&
      parsed.value === wantedValue
    ) {
      removed = true;
      continue;
    }
    toggled.push(entry);
  }
  if (!removed) toggled.push(selectionEntry(wantedKey, wantedValue));
  return toggled;
}

/** Drops every selection of `key`, keeping the other facets' selections. */
export function selectionAfterClearKey(
  selected: readonly string[],
  key: string,
): string[] {
  const wantedKey = key.trim();
  if (wantedKey.length === 0) return [...selected];
  return selected.filter((entry) => {
    const parsed = splitSelection(entry);
    return parsed === null || !sameKey(parsed.key, wantedKey);
  });
}

/** Drops every facet selection. */
export function selectionAfterClear(): string[] {
  return [];
}
