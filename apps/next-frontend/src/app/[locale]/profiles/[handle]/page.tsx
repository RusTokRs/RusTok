/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import type { Metadata } from 'next';
import { storefrontGraphql } from '@/shared/lib/graphql';
import { getStorefrontTenantSlug } from '@/shared/api/modules';
import { buildSeoMetadata } from '@/shared/seo/metadata';
import { resolveSeoPageContextForRoute } from '@/shared/seo/runtime';
import {
  ProfilesView,
  loadProfilesStorefrontPage
} from '@rustok/profiles-frontend';

interface ProfileHandlePageProps {
  params: Promise<{ locale: string; handle: string }>;
}

export async function generateMetadata({
  params,
}: ProfileHandlePageProps): Promise<Metadata> {
  const { locale, handle } = await params;
  const cleanHandle = decodeURIComponent(handle).replace(/^@/, '');
  const tenantSlug = getStorefrontTenantSlug();
  const page = await loadProfilesStorefrontPage(
    storefrontGraphql,
    cleanHandle,
    locale,
    null,
    tenantSlug
  ).catch(() => null);

  const profile = page?.profile;
  const isRu = locale === 'ru';
  const path = `/profiles/${encodeURIComponent(cleanHandle)}`;
  const seoResolution = await resolveSeoPageContextForRoute({
    locale,
    route: path,
  });

  const title = profile
    ? `${profile.displayName} (@${profile.handle}) | RusToK`
    : isRu
      ? 'Профиль пользователя | RusToK'
      : 'User Profile | RusToK';

  const description =
    profile?.bio ||
    (isRu
      ? `Публичный профиль @${cleanHandle}`
      : `Public profile of @${cleanHandle}`);

  return buildSeoMetadata({
    locale,
    title,
    description,
    path,
    context: seoResolution.context,
  });
}

export default async function ProfileHandleRoutePage({
  params,
}: ProfileHandlePageProps) {
  const { locale, handle } = await params;
  const cleanHandle = decodeURIComponent(handle).replace(/^@/, '');
  const tenantSlug = getStorefrontTenantSlug();

  const initialPage = await loadProfilesStorefrontPage(
    storefrontGraphql,
    cleanHandle,
    locale,
    null,
    tenantSlug
  ).catch(() => null);

  return (
    <main className='mx-auto max-w-5xl px-4 py-8 sm:px-6 lg:px-8'>
      <ProfilesView
        initialHandle={cleanHandle}
        initialPage={initialPage}
        graphql={storefrontGraphql}
        tenantSlug={tenantSlug}
        locale={locale}
      />
    </main>
  );
}
