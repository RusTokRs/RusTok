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
 * Calculates estimated reading time in minutes based on words per minute (WPM).
 * Defaults to 200 WPM, which is the standard average silent reading speed.
 */
export function calculateReadingTime(plainText: string, wordsPerMinute = 200): number {
  if (!plainText) {
    return 1;
  }
  const words = plainText.trim().split(/\s+/).filter(Boolean).length;
  return Math.max(1, Math.ceil(words / (Number.isFinite(wordsPerMinute) && wordsPerMinute > 0 ? wordsPerMinute : 200)));
}

/** Format through the host's canonical Fluent translator, never a language branch. */
export function formatReadingTime(
  minutes: number,
  translate: (key: 'readingTime', args: { minutes: number }) => string
): string {
  const safeMinutes = Number.isFinite(minutes) ? Math.max(1, Math.round(minutes)) : 1;
  return translate('readingTime', { minutes: safeMinutes });
}
