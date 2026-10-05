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

interface ProfilesPageProps {
  params: Promise<{ locale: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}

export async function generateMetadata({
  params,
}: {
  params: Promise<{ locale: string }>;
}): Promise<Metadata> {
  const { locale } = await params;
  const isRu = locale === 'ru';
  const path = '/profiles';
  const seoResolution = await resolveSeoPageContextForRoute({
    locale,
    route: path,
  });

  return buildSeoMetadata({
    locale,
    title: isRu ? 'Профили пользователей | RusToK' : 'Public Profiles | RusToK',
    description: isRu
      ? 'Поиск и подписка на открытые профили пользователей'
      : 'Discover and follow public user profiles',
    path,
    context: seoResolution.context,
  });
}

export default async function ProfilesRoutePage({
  params,
  searchParams,
}: ProfilesPageProps) {
  const { locale } = await params;
  const query = await searchParams;
  const tenantSlug = getStorefrontTenantSlug();

  const handle =
    typeof query.handle === 'string'
      ? query.handle.trim().replace(/^@/, '')
      : null;

  const initialPage = handle
    ? await loadProfilesStorefrontPage(
        storefrontGraphql,
        handle,
        locale,
        null,
        tenantSlug
      ).catch(() => null)
    : null;

  return (
    <main className='mx-auto max-w-5xl px-4 py-8 sm:px-6 lg:px-8'>
      <ProfilesView
        initialHandle={handle}
        initialPage={initialPage}
        graphql={storefrontGraphql}
        tenantSlug={tenantSlug}
        locale={locale}
      />
    </main>
  );
}
