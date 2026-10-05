/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

export type NotificationItemState = 'UNREAD' | 'SEEN' | 'READ' | 'ARCHIVED';
export type NotificationPriority = 'LOW' | 'NORMAL' | 'HIGH' | 'URGENT';
export type NotificationGroupStateAction = 'MARK_READ' | 'MARK_UNREAD' | 'ARCHIVE';

export interface NotificationTemplateDatum {
  key: string;
  value: string;
}

export interface NotificationStorefrontItem {
  id: string;
  source: string;
  notificationType: string;
  templateKey: string;
  actorId?: string | null;
  priority: NotificationPriority;
  state: NotificationItemState;
  templateData: NotificationTemplateDatum[];
  seenAt?: string | null;
  readAt?: string | null;
  archivedAt?: string | null;
  createdAt: string;
}

export interface NotificationStorefrontGroupSummary {
  groupKey: string;
  itemCount: number;
  unreadCount: number;
  latestItem?: NotificationStorefrontItem | null;
}

export interface NotificationStorefrontInboxSnapshot {
  unreadCount: number;
  groups: NotificationStorefrontGroupSummary[];
  nextCursor?: string | null;
  hasMore: boolean;
}

export interface NotificationGroupItemsPage {
  items: NotificationStorefrontItem[];
  nextCursor?: string | null;
  hasMore: boolean;
}

export interface NotificationGroupStateResult {
  scanned: number;
  changed: number;
  nextCursor?: string | null;
  hasMore: boolean;
}
