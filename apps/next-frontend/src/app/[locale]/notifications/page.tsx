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
  NotificationsView,
  loadInboxSnapshot
} from '@rustok/notifications-frontend';

interface NotificationsPageProps {
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
  const path = '/notifications';
  const seoResolution = await resolveSeoPageContextForRoute({
    locale,
    route: path,
  });

  return buildSeoMetadata({
    locale,
    title: isRu ? 'Центр уведомлений | RusToK' : 'Notification Inbox | RusToK',
    description: isRu
      ? 'Управляйте входящими уведомлениями, системными оповещениями и событиями платформы RusToK'
      : 'Manage incoming notifications, system alerts, and platform events on RusToK',
    path,
    context: seoResolution.context,
    noindex: true,
  });
}

export default async function NotificationsRoutePage({
  params,
}: NotificationsPageProps) {
  const { locale } = await params;
  const tenantSlug = getStorefrontTenantSlug();

  const initialSnapshot = await loadInboxSnapshot(
    storefrontGraphql,
    null,
    tenantSlug
  ).catch(() => null);

  return (
    <main className='mx-auto max-w-7xl px-4 py-8 sm:px-6 lg:px-8'>
      <NotificationsView
        initialSnapshot={initialSnapshot}
        graphql={storefrontGraphql}
        tenantSlug={tenantSlug}
        locale={locale}
      />
    </main>
  );
}
