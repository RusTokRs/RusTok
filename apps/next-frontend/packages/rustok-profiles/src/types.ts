/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

export interface ProfilesStorefrontImage {
  url: string;
  alt: string | null;
  width?: number | null;
  height?: number | null;
  mimeType?: string | null;
}

export interface ProfilesStorefrontProfile {
  userId: string;
  handle: string;
  displayName: string;
  bio: string | null;
  tags: string[];
  avatarMediaId: string | null;
  bannerMediaId: string | null;
  avatarImage: ProfilesStorefrontImage | null;
  bannerImage: ProfilesStorefrontImage | null;
  preferredLocale: string | null;
  visibility: string;
}

export interface ProfilesStorefrontFollowState {
  userId: string;
  following: boolean;
  revision: string | null;
}

export interface ProfilesStorefrontPage {
  profile: ProfilesStorefrontProfile | null;
  followState: ProfilesStorefrontFollowState | null;
  viewerAuthenticated: boolean;
  isSelf: boolean;
}

export interface SetProfilesStorefrontFollowCommand {
  userId: string;
  following: boolean;
  expectedRevision?: string | null;
  idempotencyKey: string;
}
