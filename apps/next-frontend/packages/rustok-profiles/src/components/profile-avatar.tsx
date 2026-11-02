/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import React from 'react';
import type { ProfilesStorefrontImage } from '../types';
import { getProfileInitials } from '../avatar';

export { getProfileInitials };

export function ProfileAvatar({
  image,
  displayName,
  handle,
  className
}: {
  image?: ProfilesStorefrontImage | null;
  displayName: string;
  handle: string;
  className?: string;
}): React.JSX.Element {
  const initials = getProfileInitials({ displayName, handle });

  if (image?.url) {
    return (
      <div
        className={`-mt-16 flex h-24 w-24 shrink-0 items-center justify-center overflow-hidden rounded-[1.75rem] border-4 border-card bg-primary text-2xl font-bold text-primary-foreground shadow-lg ${
          className ?? ''
        }`}
      >
        <img
          src={image.url}
          alt={image.alt ?? displayName}
          className='h-full w-full object-cover'
          decoding='async'
        />
      </div>
    );
  }

  return (
    <div
      role='img'
      aria-label={displayName}
      className={`-mt-16 flex h-24 w-24 shrink-0 items-center justify-center overflow-hidden rounded-[1.75rem] border-4 border-card bg-primary text-2xl font-bold text-primary-foreground shadow-lg ${
        className ?? ''
      }`}
    >
      <span aria-hidden='true'>{initials}</span>
    </div>
  );
}
