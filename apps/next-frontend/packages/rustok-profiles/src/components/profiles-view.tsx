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

import React, { useEffect, useState, useTransition } from 'react';
import { useRouter } from 'next/navigation';
import type { storefrontGraphql } from '@/shared/lib/graphql';
import type { ProfilesStorefrontPage } from '../types';
import { loadProfilesStorefrontPage } from '../api';
import { ProfilePanel } from './profile-panel';

type GraphqlExecutor = typeof storefrontGraphql;

export interface ProfilesViewProps {
  initialHandle?: string | null;
  initialPage?: ProfilesStorefrontPage | null;
  graphql: GraphqlExecutor;
  token?: string | null;
  tenantSlug?: string | null;
  locale?: string | null;
  viewerUserId?: string | null;
}

export function ProfilesView({
  initialHandle = null,
  initialPage = null,
  graphql,
  token,
  tenantSlug,
  locale = 'en',
  viewerUserId
}: ProfilesViewProps): React.JSX.Element {
  const router = useRouter();
  const [handleInput, setHandleInput] = useState(initialHandle ?? '');
  const [activeHandle, setActiveHandle] = useState(initialHandle);
  const [page, setPage] = useState<ProfilesStorefrontPage | null>(initialPage);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [_isPending, startTransition] = useTransition();

  const isRu = locale === 'ru';

  const badge = isRu ? 'Публичный профиль' : 'Public profile';
  const title = isRu ? 'Поиск профилей' : 'Discover people';
  const body = isRu
    ? 'Найдите профиль по имени пользователя и подписывайтесь на обновления.'
    : 'Open a profile by handle and follow people whose updates you want to see.';
  const searchLabel = isRu ? 'Имя пользователя' : 'Profile handle';
  const searchPlaceholder = isRu ? 'alice' : 'alice';
  const searchAction = isRu ? 'Открыть профиль' : 'Open profile';
  const loadErrorMsg = isRu
    ? 'Не удалось загрузить профиль'
    : 'Failed to load profile';

  const handleSubmit = (event: React.FormEvent) => {
    event.preventDefault();
    const trimmed = handleInput.trim().replace(/^@/, '');
    if (!trimmed) return;

    setActiveHandle(trimmed);
    startTransition(() => {
      router.push(`/${locale}/profiles/${encodeURIComponent(trimmed)}`);
    });
  };

  useEffect(() => {
    if (!activeHandle) {
      setPage(null);
      return;
    }

    if (initialPage && initialHandle === activeHandle) {
      setPage(initialPage);
      return;
    }

    let cancelled = false;
    setLoading(true);
    setError(null);

    loadProfilesStorefrontPage(
      graphql,
      activeHandle,
      locale,
      token,
      tenantSlug,
      viewerUserId
    )
      .then((data) => {
        if (!cancelled) {
          setPage(data);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(err?.message ?? loadErrorMsg);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [activeHandle, initialHandle, initialPage, graphql, locale, token, tenantSlug, viewerUserId]);

  const searchForm = (
    <form
      onSubmit={handleSubmit}
      className='mt-6 flex flex-col gap-3 sm:flex-row sm:items-end'
    >
      <label className='flex-1 text-sm font-medium text-foreground'>
        <span className='mb-2 block'>{searchLabel}</span>
        <input
          name='handle'
          value={handleInput}
          onChange={(e) => setHandleInput(e.target.value)}
          placeholder={searchPlaceholder}
          autoComplete='off'
          autoCapitalize='none'
          spellCheck='false'
          className='w-full rounded-2xl border border-border bg-background px-4 py-3 text-foreground outline-none transition focus:border-primary'
        />
      </label>
      <button
        type='submit'
        className='inline-flex min-h-12 items-center justify-center rounded-2xl bg-primary px-5 py-3 text-sm font-semibold text-primary-foreground transition hover:opacity-90'
      >
        {searchAction}
      </button>
    </form>
  );

  return (
    <section className='space-y-6'>
      <header className='rounded-[2rem] border border-border bg-card p-6 shadow-sm sm:p-8'>
        <span className='inline-flex rounded-full border border-border px-3 py-1 text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground'>
          {badge}
        </span>
        <h1 className='mt-3 text-2xl font-semibold text-card-foreground sm:text-3xl'>
          {title}
        </h1>
        <p className='mt-2 max-w-2xl text-sm leading-6 text-muted-foreground'>
          {body}
        </p>
        {searchForm}
      </header>

      {loading ? (
        <div role='status' aria-live='polite'>
          <div
            className='h-80 animate-pulse rounded-[2rem] bg-muted'
            aria-hidden='true'
          />
          <span className='sr-only'>
            {isRu ? 'Загрузка профиля...' : 'Loading profile...'}
          </span>
        </div>
      ) : error ? (
        <div
          role='alert'
          className='rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive'
        >
          {error}
        </div>
      ) : page ? (
        <ProfilePanel
          page={page}
          graphql={graphql}
          token={token}
          tenantSlug={tenantSlug}
          locale={locale}
        />
      ) : null}
    </section>
  );
}
