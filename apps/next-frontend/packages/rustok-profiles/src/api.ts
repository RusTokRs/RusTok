/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import type { storefrontGraphql } from '@/shared/lib/graphql';
import type {
  ProfilesStorefrontFollowState,
  ProfilesStorefrontPage,
  ProfilesStorefrontProfile,
  SetProfilesStorefrontFollowCommand
} from './types';

type GraphqlExecutor = typeof storefrontGraphql;

export const PROFILE_QUERY = `
  query ProfilesStorefrontProfile($handle: String!, $locale: String) {
    profile: profileByHandle(handle: $handle, locale: $locale) {
      userId
      handle
      displayName
      bio
      tags
      avatarMediaId
      bannerMediaId
      avatarImage { url alt width height mimeType }
      bannerImage { url alt width height mimeType }
      preferredLocale
      visibility
    }
  }
`;

export const FOLLOW_STATE_QUERY = `
  query ProfilesStorefrontFollowState($userId: UUID!) {
    state: followState(userId: $userId) {
      userId
      following
      revision
    }
  }
`;

export const FOLLOW_MUTATION = `
  mutation ProfilesStorefrontFollow($idempotencyKey: String!, $userId: UUID!, $expectedRevision: String) {
    state: followUser(idempotencyKey: $idempotencyKey, userId: $userId, expectedRevision: $expectedRevision) {
      userId
      following
      revision
    }
  }
`;

export const UNFOLLOW_MUTATION = `
  mutation ProfilesStorefrontUnfollow($idempotencyKey: String!, $userId: UUID!, $expectedRevision: String) {
    state: unfollowUser(idempotencyKey: $idempotencyKey, userId: $userId, expectedRevision: $expectedRevision) {
      userId
      following
      revision
    }
  }
`;

export async function fetchProfileByHandle(
  graphql: GraphqlExecutor,
  handle: string,
  locale?: string | null,
  token?: string | null,
  tenantSlug?: string | null
): Promise<ProfilesStorefrontProfile | null> {
  const result = await graphql<{ profile: ProfilesStorefrontProfile | null }>({
    query: PROFILE_QUERY,
    variables: { handle, locale: locale ?? undefined },
    token: token ?? undefined,
    tenant: tenantSlug ?? undefined
  });

  return result.data?.profile ?? null;
}

export async function fetchFollowState(
  graphql: GraphqlExecutor,
  userId: string,
  token?: string | null,
  tenantSlug?: string | null
): Promise<ProfilesStorefrontFollowState | null> {
  const result = await graphql<{ state: ProfilesStorefrontFollowState | null }>({
    query: FOLLOW_STATE_QUERY,
    variables: { userId },
    token: token ?? undefined,
    tenant: tenantSlug ?? undefined
  });

  return result.data?.state ?? null;
}

export async function setProfilesStorefrontFollow(
  graphql: GraphqlExecutor,
  command: SetProfilesStorefrontFollowCommand,
  token?: string | null,
  tenantSlug?: string | null
): Promise<ProfilesStorefrontFollowState> {
  const query = command.following ? FOLLOW_MUTATION : UNFOLLOW_MUTATION;
  const result = await graphql<{ state: ProfilesStorefrontFollowState }>({
    query,
    variables: {
      idempotencyKey: command.idempotencyKey,
      userId: command.userId,
      expectedRevision: command.expectedRevision ?? undefined
    },
    token: token ?? undefined,
    tenant: tenantSlug ?? undefined
  });

  if (result.errors?.length || !result.data?.state) {
    throw new Error(result.errors?.[0]?.message ?? 'Failed to update follow state');
  }

  return result.data.state;
}

export async function loadProfilesStorefrontPage(
  graphql: GraphqlExecutor,
  handle: string,
  locale?: string | null,
  token?: string | null,
  tenantSlug?: string | null,
  viewerUserId?: string | null
): Promise<ProfilesStorefrontPage> {
  const profile = await fetchProfileByHandle(graphql, handle, locale, token, tenantSlug);
  if (!profile) {
    return {
      profile: null,
      followState: null,
      viewerAuthenticated: Boolean(token),
      isSelf: false
    };
  }

  const isSelf = Boolean(viewerUserId && viewerUserId === profile.userId);
  let followState: ProfilesStorefrontFollowState | null = null;

  if (token && !isSelf) {
    followState = await fetchFollowState(graphql, profile.userId, token, tenantSlug).catch(() => null);
  }

  return {
    profile,
    followState,
    viewerAuthenticated: Boolean(token),
    isSelf
  };
}
