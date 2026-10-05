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

export function ProfileBanner({
  image
}: {
  image?: ProfilesStorefrontImage | null;
}): React.JSX.Element {
  if (image?.url) {
    return (
      <img
        src={image.url}
        alt={image.alt ?? ''}
        className='h-32 w-full object-cover'
        decoding='async'
      />
    );
  }

  return (
    <div
      className='h-32 bg-gradient-to-r from-primary/25 via-muted to-primary/10'
      aria-hidden='true'
    />
  );
}
