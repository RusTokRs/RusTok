/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import type { ProfilesStorefrontProfile } from './types';

export function getProfileInitials(
  profile: Pick<ProfilesStorefrontProfile, 'displayName' | 'handle'>
): string {
  const parts = profile.displayName.trim().split(/\s+/).filter(Boolean);
  const initials = parts
    .slice(0, 2)
    .map((part) => part[0])
    .join('')
    .toUpperCase();

  if (initials) {
    return initials;
  }

  const handle = profile.handle.trim();
  return handle ? handle[0].toUpperCase() : '?';
}
