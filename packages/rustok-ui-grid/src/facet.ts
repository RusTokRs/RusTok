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
 * counted and the `key=value` selection vocabulary the panel mutates. It deliberately knows
 * nothing about React, Next, routing or the HTTP layer.
 */

import type { FilterOption } from './filter';

/** Separator between the key and value of one selection entry (`code=value`). */
export const SELECTION_SEPARATOR = '=';

/** Separator between selection entries inside one route value (`color=red;size=m`). */
export const SELECTION_LIST_SEPARATOR = ';';

/** Maximum number of facets one grid panel renders. */
export const MAX_GRID_FACETS = 8;

/** Maximum number of bucket values one enumerable facet carries. */
export const MAX_GRID_FACET_VALUES = 20;

/** What a facet's value domain is, mirroring `rustok_grid::FacetDomain`. */
export type FacetDomain =
  | { kind: 'dictionary'; multi: boolean }
  | { kind: 'boolean' }
  | { kind: 'open' };

/** One counted bucket of an enumerable facet — the twin of Rust's `FacetValue`. */
export type FacetValue = {
  value: string;
  label: string;
  count: number;
};

/**
 * A facet an owner resolved for the current filter set, as the panel renders it.
 */
export type FacetSource = {
  code: string;
  label: string;
  domain: FacetDomain;
  /**
   * Derived from the domain by `normalizeFacetSource`; a value supplied by an owner is never
   * trusted, and any disagreement with the domain is repaired rather than validated.
   */
  isEnumerable: boolean;
  /** True when the bucket list was cut at `MAX_GRID_FACET_VALUES`. */
  isTruncated: boolean;
  /** Number of rows matching every other active facet that carry a value for this facet. */
  total: number;
  values: FacetValue[];
};

/**
 * The facet shape an owner hands over.
 *
 * `isEnumerable` is accepted for back-compat with snapshots that carried it and is always derived
 * from the domain; nothing about the panel's behaviour may depend on what an owner supplied.
 */
export type FacetSourceInput = Omit<FacetSource, "isEnumerable"> & { isEnumerable?: boolean };

function normalizeFacetCount(value: number, name: string): number {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new RangeError(`${name} must be a non-negative safe integer`);
  }
  return value;
}

function normalizeFacetDomain(domain: FacetDomain): FacetDomain {
  if (typeof domain !== 'object' || domain === null || typeof domain.kind !== 'string') {
    throw new TypeError('facet domain must be a valid domain object');
  }
  if (domain.kind === 'dictionary' && typeof domain.multi === 'boolean') {
    return { kind: 'dictionary', multi: domain.multi };
  }
  if (domain.kind === 'boolean') return { kind: 'boolean' };
  if (domain.kind === 'open') return { kind: 'open' };
  throw new TypeError('unknown or malformed facet domain');
}

function normalizeFacetText(value: string, name: string): string {
  if (typeof value !== 'string') throw new TypeError(`${name} must be a string`);
  return value;
}

/** Create a dictionary domain. */
export function dictionaryDomain(multi: boolean): FacetDomain {
  return { kind: 'dictionary', multi };
}

/** Create a boolean domain. */
export function booleanDomain(): FacetDomain {
  return { kind: 'boolean' };
}

/** Create an unbounded domain. */
export function openDomain(): FacetDomain {
  return { kind: 'open' };
}

/** True when a domain has a bounded, enumerable value set. */
export function facetDomainIsEnumerable(domain: FacetDomain): boolean {
  if (domain.kind === 'open') return false;
  if (domain.kind === 'dictionary' || domain.kind === 'boolean') return true;
  throw new TypeError('unknown facet domain');
}

/**
 * Normalize a facet received from an owner or transport. Open domains never carry buckets;
 * enumerable domains are capped and report truncation. Bucket records are copied so later mutation
 * of the owner's input cannot rewrite the panel's snapshot.
 */
export function normalizeFacetSource(facet: FacetSourceInput): FacetSource {
  if (typeof facet !== 'object' || facet === null) {
    throw new TypeError('facet source must be an object');
  }
  if (!Array.isArray(facet.values)) throw new TypeError('facet values must be an array');
  if (typeof facet.isTruncated !== 'boolean') {
    throw new TypeError('facet truncation state must be a boolean');
  }

  const domain = normalizeFacetDomain(facet.domain);
  const isEnumerable = facetDomainIsEnumerable(domain);
  const values: FacetValue[] = [];
  let isTruncated = false;

  if (isEnumerable) {
    for (const bucket of facet.values) {
      if (values.length === MAX_GRID_FACET_VALUES) {
        isTruncated = true;
        break;
      }
      if (typeof bucket !== 'object' || bucket === null) {
        throw new TypeError('facet bucket must be an object');
      }
      values.push({
        value: normalizeFacetText(bucket.value, 'facet bucket value'),
        label: normalizeFacetText(bucket.label, 'facet bucket label'),
        count: normalizeFacetCount(bucket.count, 'facet bucket count'),
      });
    }
    isTruncated ||= facet.isTruncated;
  }

  return {
    code: normalizeFacetText(facet.code, 'facet code'),
    label: normalizeFacetText(facet.label, 'facet label'),
    domain,
    isEnumerable,
    isTruncated,
    total: normalizeFacetCount(facet.total, 'facet total'),
    values,
  };
}

/** Build a facet from counted buckets, enforcing the shared cap and open-domain rule. */
export function facetFromBuckets(input: {
  code: string;
  label: string;
  domain: FacetDomain;
  total: number;
  values: readonly FacetValue[];
}): FacetSource {
  return normalizeFacetSource({
    code: input.code,
    label: input.label,
    domain: input.domain,
    isEnumerable: facetDomainIsEnumerable(input.domain),
    isTruncated: false,
    total: input.total,
    values: facetDomainIsEnumerable(input.domain)
      ? input.values.slice(0, MAX_GRID_FACET_VALUES + 1)
      : [],
  });
}

/** Build an unbounded facet: the owner reported a total, the domain has no buckets. */
export function openFacet(input: {
  code: string;
  label: string;
  total: number;
}): FacetSource {
  return normalizeFacetSource({
    code: input.code,
    label: input.label,
    domain: openDomain(),
    isEnumerable: false,
    isTruncated: false,
    total: input.total,
    values: [],
  });
}

/** The counted buckets as select-filter options; counts remain facet-owned. */
export function facetFilterOptions(facet: FacetSourceInput): FilterOption[] {
  return normalizeFacetSource(facet).values.map(({ value, label }) => ({ value, label }));
}

/** The `key=value` selection entry a bucket toggles. */
export function selectionEntry(key: string, value: string): string {
  return `${key}${SELECTION_SEPARATOR}${value}`;
}

/** The key part of one valid selection entry, or null when it addresses nothing. */
export function selectionKey(entry: string): string | null {
  const separator = entry.indexOf(SELECTION_SEPARATOR);
  if (separator < 0) return null;
  const key = entry.slice(0, separator).trim();
  const value = entry.slice(separator + 1).trim();
  return key.length > 0 && value.length > 0 ? key : null;
}

/** Split one selection entry into its parts; both must be non-empty after trimming. */
export function splitSelection(entry: string): { key: string; value: string } | null {
  const separator = entry.indexOf(SELECTION_SEPARATOR);
  if (separator < 0) return null;
  const key = entry.slice(0, separator).trim();
  const value = entry.slice(separator + 1).trim();
  if (key.length === 0 || value.length === 0) return null;
  return { key, value };
}

function asciiLowercase(value: string): string {
  return value.replace(/[A-Z]/g, (character) => character.toLowerCase());
}

function sameKey(candidate: string, wanted: string): boolean {
  // Rust's `eq_ignore_ascii_case` folds ASCII only; JavaScript `toLowerCase()` folds Unicode too.
  return asciiLowercase(candidate) === asciiLowercase(wanted);
}

/** Read the list-joined route value back into selection entries. */
export function parseSelection(value?: string | null): string[] {
  const normalized = (value ?? '').trim();
  if (normalized.length === 0) return [];
  return normalized
    .split(SELECTION_LIST_SEPARATOR)
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);
}

/** Serialize selection entries back into one route value, preserving order. */
export function serializeSelection(selected: readonly string[]): string {
  return selected.join(SELECTION_LIST_SEPARATOR);
}

/**
 * True when `key=value` is selected. Keys compare ASCII-case-insensitively while values compare
 * exactly, because a value is an option id.
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
    return parsed !== null && sameKey(parsed.key, wantedKey) && parsed.value === wantedValue;
  });
}

/** True when any valid value of `key` is selected; keys compare ASCII-case-insensitively. */
export function hasSelectionForKey(selected: readonly string[], key: string): boolean {
  const wantedKey = key.trim();
  if (wantedKey.length === 0) return false;
  return selected.some((entry) => {
    const parsed = splitSelection(entry);
    return parsed !== null && sameKey(parsed.key, wantedKey);
  });
}

/** The selected entries that belong to `key`, in their original order. */
export function selectionForKey(selected: readonly string[], key: string): string[] {
  const wantedKey = key.trim();
  if (wantedKey.length === 0) return [];
  return selected.filter((entry) => {
    const parsed = splitSelection(entry);
    return parsed !== null && sameKey(parsed.key, wantedKey);
  });
}

/** The selection an owner must keep while counting `ownKey` (drill-down counts). */
export function selectionExcept(selected: readonly string[], ownKey: string): string[] {
  const wantedKey = ownKey.trim();
  return selected.filter((entry) => {
    const parsed = splitSelection(entry);
    return parsed !== null && !sameKey(parsed.key, wantedKey);
  });
}

/**
 * Flip `key=value`: an active entry (including malformed duplicates of the same selection) is
 * removed; an inactive entry is appended. Every unrelated valid entry keeps its original order.
 */
export function selectionAfterToggle(
  selected: readonly string[],
  key: string,
  value: string,
): string[] {
  const wantedKey = key.trim();
  const wantedValue = value.trim();
  if (wantedKey.length === 0 || wantedValue.length === 0) return [...selected];

  let found = false;
  const toggled: string[] = [];
  for (const entry of selected) {
    const parsed = splitSelection(entry);
    if (parsed !== null && sameKey(parsed.key, wantedKey) && parsed.value === wantedValue) {
      found = true;
      continue;
    }
    if (parsed !== null) toggled.push(entry);
  }
  if (!found) toggled.push(selectionEntry(wantedKey, wantedValue));
  return toggled;
}

/** Drop every selection of `key`, keeping other valid facet selections in order. */
export function selectionAfterClearKey(selected: readonly string[], key: string): string[] {
  const wantedKey = key.trim();
  if (wantedKey.length === 0) {
    return selected.filter((entry) => splitSelection(entry) !== null);
  }
  return selected.filter((entry) => {
    const parsed = splitSelection(entry);
    return parsed !== null && !sameKey(parsed.key, wantedKey);
  });
}

/** Drop every facet selection. */
export function selectionAfterClear(): string[] {
  return [];
}
