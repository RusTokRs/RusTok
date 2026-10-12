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
 * Ordering helpers shared by the modules of this package.
 *
 * Internal: not re-exported from the package entry point, because it answers a JavaScript-specific
 * question. Rust compares `String` by UTF-8 bytes, which *is* code-point order, so `Ord` on `str`
 * is the whole implementation there. In TypeScript the operators compare UTF-16 code units, which
 * diverges from code-point order for characters outside the Basic Multilingual Plane, and `<` on a
 * `Set` iteration is exactly the kind of silent divergence a persisted key order must not have.
 */

/** Compare two strings by Unicode code point, the order Rust's `str` ordering gives. */
export function compareCodePoints(left: string, right: string): number {
  const leftPoints = Array.from(left, (character) => character.codePointAt(0) ?? 0);
  const rightPoints = Array.from(right, (character) => character.codePointAt(0) ?? 0);
  const length = Math.min(leftPoints.length, rightPoints.length);
  for (let index = 0; index < length; index += 1) {
    const difference = leftPoints[index] - rightPoints[index];
    if (difference !== 0) return difference;
  }
  return leftPoints.length - rightPoints.length;
}
