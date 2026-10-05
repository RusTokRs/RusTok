/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

'use client';

import React from 'react';

export function NotificationUnreadBadge({
  unreadCount,
  locale = 'en'
}: {
  unreadCount: number;
  locale?: string | null;
}): React.JSX.Element {
  const isRu = locale === 'ru';
  const label =
    unreadCount === 0
      ? isRu
        ? 'Нет непрочитанных'
        : 'No unread notifications'
      : isRu
        ? `${unreadCount} непрочит.`
        : `${unreadCount} unread`;

  return (
    <span
      data-notification-unread-count={unreadCount.toString()}
      className={`inline-flex items-center rounded-full border px-3 py-1 text-xs font-semibold ${
        unreadCount > 0
          ? 'border-primary/30 bg-primary/10 text-primary'
          : 'border-border bg-background text-muted-foreground'
      }`}
    >
      {label}
    </span>
  );
}
