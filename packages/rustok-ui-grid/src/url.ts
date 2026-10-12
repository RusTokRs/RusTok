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

/**
 * Split a route target into everything before the fragment and the `#fragment` itself.
 *
 * A fragment is part of the URL, not part of the last query value. Without this split
 * `/products?color=red#reviews` parses `color` as `red#reviews`, that poisoned value is sent to
 * the API as a real filter term, and the anchor is silently destroyed on the way back out.
 * The Rust surfaces strip the fragment the same way in `apply_ui_query_pairs`.
 */
function splitFragment(target: string): [before: string, fragment: string] {
  const index = target.indexOf("#");
  if (index < 0) return [target, ""];
  return [target.slice(0, index), target.slice(index)];
}

export function applyQueryPairs(base: string, pairs: readonly QueryPair[]): string {
  const [target, fragment] = splitFragment(base);
  const separator = target.indexOf("?");
  const path = separator >= 0 ? target.slice(0, separator) : target;
  const query = separator >= 0 ? target.slice(separator + 1) : "";
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
        for (let duplicate = merged.length - 1; duplicate > index; duplicate -= 1) {
          if (merged[duplicate][0] === key) merged.splice(duplicate, 1);
        }
      } else {
        for (let duplicate = merged.length - 1; duplicate >= 0; duplicate -= 1) {
          if (merged[duplicate][0] === key) merged.splice(duplicate, 1);
        }
      }
    } else if (nextValue.length > 0) {
      merged.push([key, nextValue]);
    }
  }
  if (merged.length === 0) return path + fragment;
  const params = new URLSearchParams();
  for (const [key, value] of merged) params.append(key, value);
  return `${path}?${params.toString()}${fragment}`;
}

/**
 * Reads one query key of a route path with an optional query string.
 *
 * A missing key and an empty key are both reported as `null`, so a caller never has to decide
 * whether an empty value means "unset". Use this only on route strings: `URLSearchParams` is
 * available in the server runtime of every Next app in this repo, and the helper has no DOM need.
 */
export function queryParam(base: string, key: string): string | null {
  const [target] = splitFragment(base);
  const separator = target.indexOf("?");
  if (separator < 0) return null;
  const value = new URLSearchParams(target.slice(separator + 1)).get(key);
  if (value === null || value.length === 0) return null;
  return value;
}
