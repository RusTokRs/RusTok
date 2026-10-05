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
import type { NotificationPriority, NotificationStorefrontItem } from '../types';

function priorityClass(priority: NotificationPriority): string {
  switch (priority) {
    case 'URGENT':
      return 'border-rose-200 bg-rose-50 text-rose-700 dark:border-rose-900/40 dark:bg-rose-950/40 dark:text-rose-300';
    case 'HIGH':
      return 'border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900/40 dark:bg-amber-950/40 dark:text-amber-300';
    case 'LOW':
      return 'border-slate-200 bg-slate-50 text-slate-600 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-400';
    default:
      return 'border-primary/20 bg-primary/5 text-primary';
  }
}

export function NotificationItemRow({
  item,
  locale = 'en'
}: {
  item: NotificationStorefrontItem;
  locale?: string | null;
}): React.JSX.Element {
  const isUnread = item.state === 'UNREAD';

  return (
    <div
      className={`rounded-2xl border p-4 transition-colors ${
        isUnread
          ? 'border-primary/40 bg-primary/5'
          : 'border-border bg-card/60'
      }`}
    >
      <div className='flex flex-wrap items-center justify-between gap-2'>
        <div className='flex items-center gap-2'>
          <span
            className={`rounded-full border px-2.5 py-0.5 text-[10px] font-semibold uppercase ${priorityClass(
              item.priority
            )}`}
          >
            {item.priority}
          </span>
          <span className='font-mono text-xs font-medium text-card-foreground'>
            {item.notificationType}
          </span>
          <span className='text-[10px] text-muted-foreground'>({item.source})</span>
        </div>
        <span className='text-xs text-muted-foreground'>
          {new Date(item.createdAt).toLocaleString(locale ?? 'en', {
            dateStyle: 'short',
            timeStyle: 'short'
          })}
        </span>
      </div>

      {item.templateData.length > 0 && (
        <div className='mt-3 flex flex-wrap gap-2'>
          {item.templateData.map((d) => (
            <span
              key={d.key}
              className='rounded-xl border border-border bg-background px-2.5 py-1 text-xs text-muted-foreground'
            >
              <strong className='text-foreground'>{d.key}:</strong> {d.value}
            </span>
          ))}
        </div>
      )}

      <div className='mt-3 flex items-center justify-between text-[11px] text-muted-foreground'>
        <span>ID: {item.id}</span>
        <span className='capitalize'>{item.state.toLowerCase()}</span>
      </div>
    </div>
  );
}
