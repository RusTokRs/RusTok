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
 * Query-string editing shared by every Next surface that keeps its table state in the route.
 *
 * This mirrors `rustok_ui_core::apply_ui_query_pairs` used by the Leptos surfaces, so both hosts
 * rewrite a URL the same way: an existing key keeps its position (stable links on repeated
 * toggles), an empty or blank value removes the key, an unknown key is appended, and values are
 * encoded with browser form rules.
 */

export type QueryPair = readonly [string, string | null | undefined];

export function applyQueryPairs(base: string, pairs: readonly QueryPair[]): string {
  const separator = base.indexOf("?");
  const path = separator >= 0 ? base.slice(0, separator) : base;
  const query = separator >= 0 ? base.slice(separator + 1) : "";
  const merged: Array<[string, string]> = [];
  for (const [key, value] of new URLSearchParams(query).entries()) {
    merged.push([key, value]);
  }
  for (const [key, rawValue] of pairs) {
    const nextValue =
      rawValue === null || rawValue === undefined ? "" : rawValue.trim();
    const index = merged.findIndex(([existingKey]) => existingKey === key);
    if (index >= 0) {
      if (nextValue.length > 0) {
        merged[index][1] = nextValue;
      } else {
        merged.splice(index, 1);
      }
    } else if (nextValue.length > 0) {
      merged.push([key, nextValue]);
    }
  }
  if (merged.length === 0) return path;
  const params = new URLSearchParams();
  for (const [key, value] of merged) params.append(key, value);
  return `${path}?${params.toString()}`;
}

/** Reads one query key of a route path with an optional query string. */
export function queryParam(base: string, key: string): string | null {
  const separator = base.indexOf("?");
  if (separator < 0) return null;
  return new URLSearchParams(base.slice(separator + 1)).get(key);
}
