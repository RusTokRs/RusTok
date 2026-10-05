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

import React, { useEffect, useState } from 'react';
import type { storefrontGraphql } from '@/shared/lib/graphql';
import type {
  NotificationGroupStateAction,
  NotificationStorefrontInboxSnapshot,
  NotificationStorefrontItem
} from '../types';
import {
  applyNotificationGroupState,
  fetchGroupItems,
  loadInboxSnapshot
} from '../api';
import { NotificationGroupCard } from './notification-group-card';
import { NotificationItemRow } from './notification-item-row';
import { NotificationUnreadBadge } from './notification-unread-badge';

type GraphqlExecutor = typeof storefrontGraphql;

export interface NotificationsViewProps {
  initialSnapshot?: NotificationStorefrontInboxSnapshot | null;
  graphql: GraphqlExecutor;
  token?: string | null;
  tenantSlug?: string | null;
  locale?: string | null;
}

export function NotificationsView({
  initialSnapshot = null,
  graphql,
  token,
  tenantSlug,
  locale = 'en'
}: NotificationsViewProps): React.JSX.Element {
  const [snapshot, setSnapshot] =
    useState<NotificationStorefrontInboxSnapshot | null>(initialSnapshot);
  const [selectedGroupKey, setSelectedGroupKey] = useState<string | null>(null);
  const [groupItems, setGroupItems] = useState<NotificationStorefrontItem[]>([]);
  const [loadingItems, setLoadingItems] = useState(false);
  const [loading, setLoading] = useState(!initialSnapshot);
  const [busyAction, setBusyAction] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const isRu = locale === 'ru';
  const badge = isRu ? 'Уведомления' : 'Notifications';
  const title = isRu ? 'Входящие уведомления' : 'Notification Inbox';
  const subtitle = isRu
    ? 'Просматривайте уведомления от системы и модулей платформы, управляйте статусами прочтения.'
    : 'View and manage system and module notifications across your channels.';

  const handleRefresh = async () => {
    setLoading(true);
    setError(null);
    setFeedback(null);
    try {
      const data = await loadInboxSnapshot(graphql, token, tenantSlug);
      setSnapshot(data);
    } catch (err: unknown) {
      setError(
        err instanceof Error
          ? err.message
          : isRu
            ? 'Ошибка загрузки уведомлений'
            : 'Failed to load notifications'
      );
    } finally {
      setLoading(false);
    }
  };

  const handleGroupAction = async (
    groupKey: string,
    action: NotificationGroupStateAction
  ) => {
    if (busyAction) return;

    setBusyAction(true);
    setFeedback(null);

    const idempotencyKey = `notif-${groupKey}-${action}-${Date.now()}`;
    try {
      await applyNotificationGroupState(
        graphql,
        {
          groupKey,
          action,
          idempotencyKey
        },
        token,
        tenantSlug
      );
      setFeedback(
        action === 'MARK_READ'
          ? isRu
            ? 'Группа отмечена как прочитанная'
            : 'Group marked as read'
          : action === 'ARCHIVE'
            ? isRu
              ? 'Группа отправлена в архив'
              : 'Group archived'
            : isRu
              ? 'Статус обновлен'
              : 'Status updated'
      );
      await handleRefresh();
      if (selectedGroupKey === groupKey) {
        loadItems(groupKey);
      }
    } catch (err: unknown) {
      setError(
        err instanceof Error
          ? err.message
          : isRu
            ? 'Не удалось выполнить действие'
            : 'Failed to apply action'
      );
    } finally {
      setBusyAction(false);
    }
  };

  const loadItems = async (groupKey: string) => {
    setLoadingItems(true);
    try {
      const res = await fetchGroupItems(
        graphql,
        groupKey,
        null,
        null,
        20,
        token,
        tenantSlug
      );
      setGroupItems(res.items);
    } catch (err: unknown) {
      setError(
        err instanceof Error
          ? err.message
          : isRu
            ? 'Ошибка загрузки элементов группы'
            : 'Failed to load group items'
      );
    } finally {
      setLoadingItems(false);
    }
  };

  const handleSelectGroup = (groupKey: string) => {
    if (selectedGroupKey === groupKey) {
      setSelectedGroupKey(null);
      setGroupItems([]);
    } else {
      setSelectedGroupKey(groupKey);
      loadItems(groupKey);
    }
  };

  useEffect(() => {
    if (!initialSnapshot) {
      handleRefresh();
    }
  }, [initialSnapshot]);

  return (
    <section className='space-y-8'>
      <header className='rounded-[2rem] border border-border bg-card p-6 shadow-sm sm:p-8'>
        <div className='flex flex-wrap items-start justify-between gap-4'>
          <div>
            <div className='flex items-center gap-3'>
              <span className='inline-flex rounded-full border border-border px-3 py-1 text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground'>
                {badge}
              </span>
              <NotificationUnreadBadge
                unreadCount={snapshot?.unreadCount ?? 0}
                locale={locale}
              />
            </div>
            <h1 className='mt-3 text-2xl font-bold text-card-foreground sm:text-3xl'>
              {title}
            </h1>
            <p className='mt-2 max-w-2xl text-sm leading-6 text-muted-foreground'>
              {subtitle}
            </p>
          </div>

          <button
            type='button'
            disabled={loading}
            onClick={handleRefresh}
            className='inline-flex items-center gap-1.5 rounded-2xl border border-border bg-background px-4 py-2 text-xs font-medium text-foreground transition hover:bg-muted disabled:opacity-50'
          >
            {loading
              ? isRu
                ? 'Обновление...'
                : 'Refreshing...'
              : isRu
                ? '↻ Обновить'
                : '↻ Refresh'}
          </button>
        </div>

        {feedback && (
          <div
            role='status'
            aria-live='polite'
            className='mt-4 rounded-xl border border-emerald-200 bg-emerald-50 px-4 py-2.5 text-xs font-medium text-emerald-800 dark:border-emerald-900/40 dark:bg-emerald-950/40 dark:text-emerald-300'
          >
            {feedback}
          </div>
        )}

        {error && (
          <div
            role='alert'
            className='mt-4 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-2.5 text-xs text-destructive'
          >
            {error}
          </div>
        )}
      </header>

      {loading && !snapshot ? (
        <div className='space-y-4'>
          <div className='h-32 animate-pulse rounded-2xl bg-muted' />
          <div className='h-32 animate-pulse rounded-2xl bg-muted' />
        </div>
      ) : snapshot ? (
        <div className='space-y-6'>
          {snapshot.groups.length === 0 ? (
            <div className='rounded-[2rem] border border-dashed border-border bg-card p-12 text-center text-sm text-muted-foreground'>
              {isRu ? 'У вас нет уведомлений' : 'You have no notifications'}
            </div>
          ) : (
            <div className='grid gap-4 md:grid-cols-2'>
              {snapshot.groups.map((group) => (
                <NotificationGroupCard
                  key={group.groupKey}
                  group={group}
                  isSelected={selectedGroupKey === group.groupKey}
                  onSelect={handleSelectGroup}
                  onAction={handleGroupAction}
                  busyAction={busyAction}
                  locale={locale}
                />
              ))}
            </div>
          )}

          {selectedGroupKey && (
            <div className='space-y-4 rounded-[2rem] border border-border bg-card p-6 shadow-sm sm:p-8'>
              <div className='flex items-center justify-between'>
                <h2 className='text-lg font-semibold text-card-foreground'>
                  {isRu
                    ? `Уведомления группы: ${selectedGroupKey}`
                    : `Group Items: ${selectedGroupKey}`}
                </h2>
                <button
                  type='button'
                  onClick={() => setSelectedGroupKey(null)}
                  className='text-xs font-medium text-muted-foreground hover:text-foreground'
                >
                  {isRu ? '✕ Закрыть' : '✕ Close'}
                </button>
              </div>

              {loadingItems ? (
                <div className='space-y-3'>
                  <div className='h-20 animate-pulse rounded-xl bg-muted' />
                  <div className='h-20 animate-pulse rounded-xl bg-muted' />
                </div>
              ) : groupItems.length === 0 ? (
                <p className='text-xs text-muted-foreground'>
                  {isRu ? 'В группе нет элементов' : 'No items in this group'}
                </p>
              ) : (
                <div className='space-y-3'>
                  {groupItems.map((item) => (
                    <NotificationItemRow
                      key={item.id}
                      item={item}
                      locale={locale}
                    />
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      ) : null}
    </section>
  );
}
