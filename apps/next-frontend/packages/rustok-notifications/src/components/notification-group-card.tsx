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
import type {
  NotificationGroupStateAction,
  NotificationStorefrontGroupSummary
} from '../types';

export function NotificationGroupCard({
  group,
  isSelected,
  onSelect,
  onAction,
  busyAction,
  locale = 'en'
}: {
  group: NotificationStorefrontGroupSummary;
  isSelected?: boolean;
  onSelect: (groupKey: string) => void;
  onAction: (groupKey: string, action: NotificationGroupStateAction) => void;
  busyAction?: boolean;
  locale?: string | null;
}): React.JSX.Element {
  const isRu = locale === 'ru';
  const hasUnread = group.unreadCount > 0;
  const latest = group.latestItem;

  return (
    <article
      className={`rounded-2xl border p-5 transition-all ${
        isSelected
          ? 'border-primary bg-primary/5 shadow-sm'
          : 'border-border bg-card hover:border-primary/40'
      }`}
    >
      <div className='flex items-start justify-between gap-3'>
        <div className='flex items-center gap-2'>
          <h3 className='text-sm font-semibold text-card-foreground'>
            {group.groupKey}
          </h3>
          {hasUnread && (
            <span className='rounded-full bg-primary/10 px-2 py-0.5 text-[10px] font-semibold text-primary'>
              {isRu
                ? `${group.unreadCount} новых`
                : `${group.unreadCount} new`}
            </span>
          )}
        </div>
        <span className='text-xs text-muted-foreground'>
          {isRu
            ? `${group.itemCount} уведомл.`
            : `${group.itemCount} items`}
        </span>
      </div>

      {latest && (
        <div className='mt-3 rounded-xl border border-border/50 bg-background/50 p-3'>
          <div className='flex items-center justify-between text-[11px] text-muted-foreground'>
            <span className='font-medium text-foreground'>
              {latest.notificationType}
            </span>
            <span>
              {new Date(latest.createdAt).toLocaleDateString(locale ?? 'en')}
            </span>
          </div>
          {latest.templateData.length > 0 && (
            <p className='mt-1 text-xs text-muted-foreground line-clamp-1'>
              {latest.templateData
                .map((d) => `${d.key}: ${d.value}`)
                .join(' · ')}
            </p>
          )}
        </div>
      )}

      <div className='mt-4 flex flex-wrap items-center justify-between gap-2 border-t border-border/50 pt-3'>
        <button
          type='button'
          onClick={() => onSelect(group.groupKey)}
          className='text-xs font-medium text-primary hover:underline'
        >
          {isSelected
            ? isRu
              ? 'Скрыть элементы'
              : 'Hide items'
            : isRu
              ? 'Просмотр элементов →'
              : 'Inspect items →'}
        </button>

        <div className='flex items-center gap-2'>
          {hasUnread && (
            <button
              type='button'
              disabled={busyAction}
              onClick={() => onAction(group.groupKey, 'MARK_READ')}
              className='rounded-xl border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition hover:bg-muted disabled:opacity-50'
            >
              {isRu ? 'Прочитано' : 'Mark Read'}
            </button>
          )}
          <button
            type='button'
            disabled={busyAction}
            onClick={() => onAction(group.groupKey, 'ARCHIVE')}
            className='rounded-xl border border-border bg-background px-2.5 py-1 text-xs font-medium text-muted-foreground transition hover:text-foreground hover:bg-muted disabled:opacity-50'
          >
            {isRu ? 'В архив' : 'Archive'}
          </button>
        </div>
      </div>
    </article>
  );
}
