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
  return Math.max(1, Math.ceil(words / wordsPerMinute));
}

/**
 * Returns a formatted, localized reading time string.
 */
export function formatReadingTime(minutes: number, locale = 'en'): string {
  if (locale === 'ru') {
    const mod10 = minutes % 10;
    const mod100 = minutes % 100;
    let suffix = 'минут';
    if (mod10 === 1 && mod100 !== 11) {
      suffix = 'минута';
    } else if (mod10 >= 2 && mod10 <= 4 && (mod100 < 10 || mod100 >= 20)) {
      suffix = 'минуты';
    }
    return `${minutes} ${suffix} чтения`;
  }
  return `${minutes} min read`;
}
