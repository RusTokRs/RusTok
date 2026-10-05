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

import React, { useState } from 'react';
import type { storefrontGraphql } from '@/shared/lib/graphql';
import type {
  ProfilesStorefrontFollowState,
  ProfilesStorefrontPage
} from '../types';
import {
  fetchFollowState,
  setProfilesStorefrontFollow
} from '../api';
import { ProfileAvatar } from './profile-avatar';
import { ProfileBanner } from './profile-banner';

type GraphqlExecutor = typeof storefrontGraphql;

export function ProfilePanel({
  page,
  graphql,
  token,
  tenantSlug,
  locale = 'en'
}: {
  page: ProfilesStorefrontPage;
  graphql: GraphqlExecutor;
  token?: string | null;
  tenantSlug?: string | null;
  locale?: string | null;
}): React.JSX.Element {
  const profile = page.profile;

  if (!profile) {
    return (
      <div
        role='status'
        className='rounded-[2rem] border border-dashed border-border bg-card p-10 text-center text-muted-foreground'
      >
        {locale === 'ru'
          ? 'Этот профиль недоступен.'
          : 'This profile is unavailable.'}
      </div>
    );
  }

  const [followState, setFollowState] =
    useState<ProfilesStorefrontFollowState | null>(page.followState);
  const [busy, setBusy] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);

  const isSelf = page.isSelf;
  const viewerAuthenticated = page.viewerAuthenticated;
  const isFollowing = followState?.following ?? false;

  const handleToggleFollow = async () => {
    if (busy) return;

    setBusy(true);
    setFeedback(null);

    const idempotencyKey = `follow-${profile.userId}-${Date.now()}`;
    const targetFollowing = !isFollowing;

    try {
      const nextState = await setProfilesStorefrontFollow(
        graphql,
        {
          userId: profile.userId,
          following: targetFollowing,
          expectedRevision: followState?.revision ?? null,
          idempotencyKey
        },
        token,
        tenantSlug
      );
      setFollowState(nextState);
    } catch (_err) {
      // On concurrency revision mismatch or network error, attempt recovery
      try {
        const recovered = await fetchFollowState(
          graphql,
          profile.userId,
          token,
          tenantSlug
        );
        if (recovered) {
          setFollowState(recovered);
          setFeedback(
            locale === 'ru'
              ? 'Состояние подписки изменилось. Мы обновили данные; попробуйте еще раз.'
              : 'The follow state changed elsewhere. We refreshed it; try again.'
          );
          return;
        }
      } catch {
        // Recovery failed, show generic error
      }

      setFeedback(
        locale === 'ru'
          ? 'Управление подпиской временно недоступно.'
          : 'Follow controls are temporarily unavailable.'
      );
    } finally {
      setBusy(false);
    }
  };

  const bioFallback =
    locale === 'ru'
      ? 'Биография пока не добавлена.'
      : 'No biography has been added yet.';

  return (
    <article className='overflow-hidden rounded-[2rem] border border-border bg-card shadow-sm'>
      <ProfileBanner image={profile.bannerImage} />
      <div className='p-6 sm:p-8'>
        <div className='flex flex-col gap-6 sm:flex-row sm:items-start sm:justify-between'>
          <div className='flex gap-4'>
            <ProfileAvatar
              image={profile.avatarImage}
              displayName={profile.displayName}
              handle={profile.handle}
            />
            <div>
              <h2 className='text-2xl font-semibold text-card-foreground'>
                {profile.displayName}
              </h2>
              <p className='mt-1 text-sm font-medium text-muted-foreground'>
                @{profile.handle}
              </p>
              <p className='mt-4 max-w-2xl text-sm leading-6 text-muted-foreground'>
                {profile.bio || bioFallback}
              </p>
            </div>
          </div>

          <div className='min-w-44 space-y-2'>
            {isSelf ? (
              <p
                role='status'
                className='rounded-2xl bg-muted px-4 py-3 text-center text-sm font-medium text-muted-foreground'
              >
                {locale === 'ru'
                  ? 'Это ваш профиль.'
                  : 'This is your profile.'}
              </p>
            ) : !viewerAuthenticated ? (
              <p
                role='status'
                className='rounded-2xl bg-muted px-4 py-3 text-center text-sm text-muted-foreground'
              >
                {locale === 'ru'
                  ? 'Войдите, чтобы подписаться.'
                  : 'Sign in to follow this profile.'}
              </p>
            ) : (
              <button
                type='button'
                disabled={busy}
                aria-pressed={isFollowing}
                aria-busy={busy}
                onClick={handleToggleFollow}
                className={`w-full rounded-2xl px-5 py-3 text-sm font-semibold transition focus-visible:ring-2 focus-visible:ring-primary focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-60 ${
                  isFollowing
                    ? 'border border-border bg-background text-foreground hover:bg-muted'
                    : 'bg-primary text-primary-foreground hover:opacity-90'
                }`}
              >
                {busy
                  ? locale === 'ru'
                    ? 'Обновление...'
                    : 'Updating...'
                  : isFollowing
                    ? locale === 'ru'
                      ? 'Отписаться'
                      : 'Unfollow'
                    : locale === 'ru'
                      ? 'Подписаться'
                      : 'Follow'}
              </button>
            )}

            {feedback && (
              <p
                role='status'
                aria-live='polite'
                className='rounded-xl border border-border bg-muted px-3 py-2 text-xs text-muted-foreground'
              >
                {feedback}
              </p>
            )}
          </div>
        </div>

        <div className='mt-8 grid gap-4 md:grid-cols-2'>
          <section className='rounded-2xl border border-border bg-background/60 p-4'>
            <p className='text-xs font-semibold uppercase tracking-[0.18em] text-muted-foreground'>
              {locale === 'ru' ? 'Видимость' : 'Visibility'}
            </p>
            <p className='mt-2 text-sm font-medium text-foreground capitalize'>
              {profile.visibility}
            </p>
          </section>

          <section className='rounded-2xl border border-border bg-background/60 p-4'>
            <p className='text-xs font-semibold uppercase tracking-[0.18em] text-muted-foreground'>
              {locale === 'ru' ? 'Интересы' : 'Interests'}
            </p>
            <div className='mt-3 flex flex-wrap gap-2'>
              {profile.tags.length > 0 ? (
                profile.tags.map((tag) => (
                  <span
                    key={tag}
                    className='rounded-full bg-muted px-3 py-1 text-xs font-medium text-muted-foreground'
                  >
                    {tag}
                  </span>
                ))
              ) : (
                <span className='text-xs text-muted-foreground'>
                  {locale === 'ru' ? 'Теги не указаны' : 'No tags specified'}
                </span>
              )}
            </div>
          </section>
        </div>
      </div>
    </article>
  );
}
