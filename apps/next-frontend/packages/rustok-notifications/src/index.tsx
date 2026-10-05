/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

export type {
  NotificationGroupItemsPage,
  NotificationGroupStateAction,
  NotificationGroupStateResult,
  NotificationItemState,
  NotificationPriority,
  NotificationStorefrontGroupSummary,
  NotificationStorefrontInboxSnapshot,
  NotificationStorefrontItem,
  NotificationTemplateDatum
} from './types';

export {
  applyNotificationGroupState,
  fetchGroupItems,
  fetchGroupSummaries,
  fetchUnreadCount,
  loadInboxSnapshot,
  GROUP_ITEMS_QUERY,
  GROUP_STATE_MUTATION,
  GROUP_SUMMARIES_QUERY,
  UNREAD_COUNT_QUERY
} from './api';

export { NotificationGroupCard } from './components/notification-group-card';
export { NotificationItemRow } from './components/notification-item-row';
export { NotificationUnreadBadge } from './components/notification-unread-badge';
export {
  NotificationsView,
  type NotificationsViewProps
} from './components/notifications-view';
