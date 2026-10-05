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
  NotificationGroupItemsPage,
  NotificationGroupStateAction,
  NotificationGroupStateResult,
  NotificationItemState,
  NotificationStorefrontGroupSummary,
  NotificationStorefrontInboxSnapshot
} from './types';

type GraphqlExecutor = typeof storefrontGraphql;

export const UNREAD_COUNT_QUERY = `
  query NotificationStorefrontNavigationUnreadCount {
    notificationInboxUnreadCount {
      unreadCount
    }
  }
`;

export const GROUP_SUMMARIES_QUERY = `
  query NotificationStorefrontGroupSummaries($cursor: String, $limit: Int) {
    notificationInboxGroupSummaries(cursor: $cursor, limit: $limit) {
      groups {
        groupKey
        itemCount
        unreadCount
        latestItem {
          id
          source
          notificationType
          templateKey
          actorId
          priority
          state
          templateData {
            key
            value
          }
          seenAt
          readAt
          archivedAt
          createdAt
        }
      }
      nextCursor
      hasMore
    }
  }
`;

export const GROUP_ITEMS_QUERY = `
  query NotificationStorefrontGroupItems(
    $groupKey: String!
    $state: NotificationInboxItemState
    $cursor: String
    $limit: Int
  ) {
    notificationInboxGroupItems(
      groupKey: $groupKey
      state: $state
      cursor: $cursor
      limit: $limit
    ) {
      items {
        id
        source
        notificationType
        templateKey
        actorId
        priority
        state
        templateData {
          key
          value
        }
        seenAt
        readAt
        archivedAt
        createdAt
      }
      nextCursor
      hasMore
    }
  }
`;

export const GROUP_STATE_MUTATION = `
  mutation NotificationStorefrontApplyGroupState(
    $groupKey: String!
    $action: NotificationInboxGroupStateAction!
    $cursor: String
    $limit: Int
    $idempotencyKey: String!
  ) {
    notificationInboxApplyGroupState(
      groupKey: $groupKey
      action: $action
      cursor: $cursor
      limit: $limit
      idempotencyKey: $idempotencyKey
    ) {
      scanned
      changed
      nextCursor
      hasMore
    }
  }
`;

export async function fetchUnreadCount(
  graphql: GraphqlExecutor,
  token?: string | null,
  tenantSlug?: string | null
): Promise<number> {
  const result = await graphql<{
    notificationInboxUnreadCount?: { unreadCount: number };
  }>({
    query: UNREAD_COUNT_QUERY,
    token: token ?? undefined,
    tenant: tenantSlug ?? undefined
  });

  return result.data?.notificationInboxUnreadCount?.unreadCount ?? 0;
}

export async function fetchGroupSummaries(
  graphql: GraphqlExecutor,
  cursor?: string | null,
  limit = 20,
  token?: string | null,
  tenantSlug?: string | null
): Promise<{
  groups: NotificationStorefrontGroupSummary[];
  nextCursor?: string | null;
  hasMore: boolean;
}> {
  const result = await graphql<{
    notificationInboxGroupSummaries?: {
      groups: NotificationStorefrontGroupSummary[];
      nextCursor?: string | null;
      hasMore: boolean;
    };
  }>({
    query: GROUP_SUMMARIES_QUERY,
    variables: { cursor: cursor ?? undefined, limit },
    token: token ?? undefined,
    tenant: tenantSlug ?? undefined
  });

  const page = result.data?.notificationInboxGroupSummaries;
  return {
    groups: page?.groups ?? [],
    nextCursor: page?.nextCursor ?? null,
    hasMore: page?.hasMore ?? false
  };
}

export async function fetchGroupItems(
  graphql: GraphqlExecutor,
  groupKey: string,
  state?: NotificationItemState | null,
  cursor?: string | null,
  limit = 20,
  token?: string | null,
  tenantSlug?: string | null
): Promise<NotificationGroupItemsPage> {
  const result = await graphql<{
    notificationInboxGroupItems?: NotificationGroupItemsPage;
  }>({
    query: GROUP_ITEMS_QUERY,
    variables: {
      groupKey,
      state: state ?? undefined,
      cursor: cursor ?? undefined,
      limit
    },
    token: token ?? undefined,
    tenant: tenantSlug ?? undefined
  });

  const page = result.data?.notificationInboxGroupItems;
  return {
    items: page?.items ?? [],
    nextCursor: page?.nextCursor ?? null,
    hasMore: page?.hasMore ?? false
  };
}

export async function applyNotificationGroupState(
  graphql: GraphqlExecutor,
  command: {
    groupKey: string;
    action: NotificationGroupStateAction;
    cursor?: string | null;
    limit?: number;
    idempotencyKey: string;
  },
  token?: string | null,
  tenantSlug?: string | null
): Promise<NotificationGroupStateResult> {
  const result = await graphql<{
    notificationInboxApplyGroupState?: NotificationGroupStateResult;
  }>({
    query: GROUP_STATE_MUTATION,
    variables: {
      groupKey: command.groupKey,
      action: command.action,
      cursor: command.cursor ?? undefined,
      limit: command.limit ?? 64,
      idempotencyKey: command.idempotencyKey
    },
    token: token ?? undefined,
    tenant: tenantSlug ?? undefined
  });

  if (result.errors?.length || !result.data?.notificationInboxApplyGroupState) {
    throw new Error(
      result.errors?.[0]?.message ?? 'Failed to apply notification group state'
    );
  }

  return result.data.notificationInboxApplyGroupState;
}

export async function loadInboxSnapshot(
  graphql: GraphqlExecutor,
  token?: string | null,
  tenantSlug?: string | null
): Promise<NotificationStorefrontInboxSnapshot> {
  const [unreadCount, summaries] = await Promise.all([
    fetchUnreadCount(graphql, token, tenantSlug).catch(() => 0),
    fetchGroupSummaries(graphql, null, 20, token, tenantSlug).catch(() => ({
      groups: [],
      nextCursor: null,
      hasMore: false
    }))
  ]);

  return {
    unreadCount,
    groups: summaries.groups,
    nextCursor: summaries.nextCursor,
    hasMore: summaries.hasMore
  };
}
