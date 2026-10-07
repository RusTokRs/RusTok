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
import { cookies } from 'next/headers';
import Link from 'next/link';
import { setRequestLocale } from '@rustok/next-fluent/server';
import { storefrontGraphql } from '@/shared/lib/graphql';
import { getStorefrontTenantSlug } from '@/shared/api/modules';
import { buildSeoMetadata } from '@/shared/seo/metadata';
import { resolveSeoPageContextForRoute } from '@/shared/seo/runtime';
import { fetchStorefrontOrders } from '@rustok/order-frontend';
import { ADMIN_TOKEN_KEY, ADMIN_USER_KEY, type AuthUser } from '@/shared/lib/auth';
import {
  Package,
  User,
  ShieldCheck,
  CreditCard,
  MapPin,
  ArrowRight,
  Clock,
  CheckCircle2,
  ExternalLink
} from 'lucide-react';

interface AccountPageProps {
  params: Promise<{ locale: string }>;
}

export async function generateMetadata({
  params,
}: AccountPageProps): Promise<Metadata> {
  const { locale } = await params;
  const isRu = locale.toLowerCase().startsWith('ru');
  const path = '/account';
  const seoResolution = await resolveSeoPageContextForRoute({
    locale,
    route: path,
  });

  return buildSeoMetadata({
    locale,
    title: isRu ? 'Личный кабинет | RusToK' : 'My Account | RusToK',
    description: isRu
      ? 'Управление профилем, просмотр истории заказов и настройки аккаунта'
      : 'Manage your profile, view order history, and configure account settings',
    path,
    context: seoResolution.context,
    noindex: true,
  });
}

export default async function AccountPage({ params }: AccountPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);
  const isRu = locale.toLowerCase().startsWith('ru');
  const tenantSlug = getStorefrontTenantSlug();

  const cookieStore = await cookies();
  const token = cookieStore.get(ADMIN_TOKEN_KEY)?.value ?? null;
  const userCookie = cookieStore.get(ADMIN_USER_KEY)?.value;

  let user: AuthUser | null = null;
  if (userCookie) {
    try {
      user = JSON.parse(decodeURIComponent(userCookie)) as AuthUser;
    } catch {
      user = null;
    }
  }

  const ordersData = token
    ? await fetchStorefrontOrders(storefrontGraphql, { perPage: 3 }, tenantSlug, token)
    : null;

  const recentOrders = ordersData?.items ?? [];

  return (
    <main className='min-h-screen bg-background'>
      <div className='mx-auto max-w-6xl px-4 py-12 sm:px-6 lg:px-8 space-y-10'>
        {/* Welcome Banner */}
        <section className='rounded-3xl border border-border bg-card/60 p-8 backdrop-blur-md shadow-sm'>
          <div className='flex flex-col gap-6 sm:flex-row sm:items-center sm:justify-between'>
            <div className='flex items-center gap-4'>
              <div className='flex h-16 w-16 items-center justify-center rounded-2xl bg-primary/10 text-primary border border-primary/20'>
                <User className='h-8 w-8' />
              </div>
              <div className='space-y-1'>
                <h1 className='text-2xl font-bold tracking-tight text-foreground sm:text-3xl'>
                  {user?.name
                    ? isRu
                      ? `Здравствуйте, ${user.name}`
                      : `Welcome, ${user.name}`
                    : isRu
                    ? 'Личный кабинет'
                    : 'Customer Account'}
                </h1>
                <p className='text-sm text-muted-foreground'>
                  {user?.email ??
                    (token
                      ? isRu
                        ? 'Авторизованный покупатель'
                        : 'Authenticated Shopper'
                      : isRu
                      ? 'Гостевая сессия'
                      : 'Guest Session')}
                </p>
              </div>
            </div>
            <div className='flex items-center gap-3'>
              <Link
                href={`/${locale}/products`}
                className='inline-flex items-center gap-2 rounded-xl bg-primary px-4 py-2.5 text-xs font-semibold text-primary-foreground shadow transition hover:bg-primary/90'
              >
                {isRu ? 'В каталог товаров' : 'Browse Catalog'}
                <ArrowRight className='h-4 w-4' />
              </Link>
            </div>
          </div>
        </section>

        {/* Quick Nav Cards Grid */}
        <section className='grid gap-6 sm:grid-cols-2 lg:grid-cols-4'>
          <Link
            href={`/${locale}/account/orders`}
            className='group rounded-2xl border border-border bg-card p-5 transition hover:border-primary/40 hover:shadow-md'
          >
            <div className='flex items-center gap-3 mb-3'>
              <div className='flex h-10 w-10 items-center justify-center rounded-xl bg-blue-500/10 text-blue-500 border border-blue-500/20'>
                <Package className='h-5 w-5' />
              </div>
              <h3 className='font-semibold text-card-foreground group-hover:text-primary transition'>
                {isRu ? 'Мои заказы' : 'My Orders'}
              </h3>
            </div>
            <p className='text-xs text-muted-foreground'>
              {isRu
                ? 'История покупок, электронные чеки и статусы отправлений'
                : 'Purchase history, receipts, and shipment progress'}
            </p>
          </Link>

          <div className='rounded-2xl border border-border bg-card p-5 opacity-80'>
            <div className='flex items-center gap-3 mb-3'>
              <div className='flex h-10 w-10 items-center justify-center rounded-xl bg-emerald-500/10 text-emerald-500 border border-emerald-500/20'>
                <MapPin className='h-5 w-5' />
              </div>
              <h3 className='font-semibold text-card-foreground'>
                {isRu ? 'Адреса доставки' : 'Addresses'}
              </h3>
            </div>
            <p className='text-xs text-muted-foreground'>
              {isRu
                ? 'Сохраненные адреса для быстрого оформления в 1 клик'
                : 'Saved shipping locations for fast 1-click checkout'}
            </p>
          </div>

          <div className='rounded-2xl border border-border bg-card p-5 opacity-80'>
            <div className='flex items-center gap-3 mb-3'>
              <div className='flex h-10 w-10 items-center justify-center rounded-xl bg-amber-500/10 text-amber-500 border border-amber-500/20'>
                <CreditCard className='h-5 w-5' />
              </div>
              <h3 className='font-semibold text-card-foreground'>
                {isRu ? 'Способы оплаты' : 'Payment Methods'}
              </h3>
            </div>
            <p className='text-xs text-muted-foreground'>
              {isRu
                ? 'Привязанные карты и счета безопасных платежей'
                : 'Linked cards and secure digital wallets'}
            </p>
          </div>

          <div className='rounded-2xl border border-border bg-card p-5 opacity-80'>
            <div className='flex items-center gap-3 mb-3'>
              <div className='flex h-10 w-10 items-center justify-center rounded-xl bg-violet-500/10 text-violet-500 border border-violet-500/20'>
                <ShieldCheck className='h-5 w-5' />
              </div>
              <h3 className='font-semibold text-card-foreground'>
                {isRu ? 'Безопасность' : 'Security'}
              </h3>
            </div>
            <p className='text-xs text-muted-foreground'>
              {isRu
                ? 'Пароль, двухфакторная аутентификация и сессии'
                : 'Password, two-factor auth, and active sessions'}
            </p>
          </div>
        </section>

        {/* Recent Orders Section */}
        <section className='rounded-3xl border border-border bg-card p-6 shadow-sm space-y-4'>
          <div className='flex items-center justify-between'>
            <div>
              <h2 className='text-lg font-bold text-foreground'>
                {isRu ? 'Недавние заказы' : 'Recent Orders'}
              </h2>
              <p className='text-xs text-muted-foreground'>
                {isRu
                  ? 'Последние операции оформления заказа'
                  : 'Latest checkout and fulfillment activity'}
              </p>
            </div>
            <Link
              href={`/${locale}/account/orders`}
              className='text-xs font-semibold text-primary hover:underline flex items-center gap-1'
            >
              {isRu ? 'Все заказы' : 'View all'}
              <ArrowRight className='h-3.5 w-3.5' />
            </Link>
          </div>

          {recentOrders.length === 0 ? (
            <div className='flex flex-col items-center justify-center py-10 text-center space-y-3 rounded-2xl border border-dashed border-border bg-muted/20'>
              <Package className='h-10 w-10 text-muted-foreground/40' />
              <p className='text-sm font-medium text-foreground'>
                {isRu ? 'Заказов пока нет' : 'No orders yet'}
              </p>
              <p className='text-xs text-muted-foreground max-w-sm'>
                {isRu
                  ? 'Когда вы оформите покупку, информация о ней появится здесь.'
                  : 'Once you complete a purchase, it will appear here.'}
              </p>
              <Link
                href={`/${locale}/products`}
                className='inline-flex items-center gap-1.5 rounded-lg border border-border bg-background px-3 py-1.5 text-xs font-medium text-foreground hover:bg-accent transition'
              >
                {isRu ? 'Перейти к покупкам' : 'Start shopping'}
              </Link>
            </div>
          ) : (
            <div className='divide-y divide-border'>
              {recentOrders.map((order) => (
                <div
                  key={order.id}
                  className='flex flex-col gap-3 py-4 sm:flex-row sm:items-center sm:justify-between'
                >
                  <div className='space-y-1'>
                    <div className='flex items-center gap-2'>
                      <span className='font-mono text-xs font-bold text-foreground'>
                        #{order.id.slice(0, 8)}
                      </span>
                      <span className='inline-flex items-center gap-1 rounded-full border border-primary/20 bg-primary/10 px-2 py-0.5 text-[11px] font-semibold text-primary capitalize'>
                        <Clock className='h-3 w-3' />
                        {order.status}
                      </span>
                    </div>
                    <p className='text-xs text-muted-foreground'>
                      {new Date(order.createdAt).toLocaleDateString(locale, {
                        year: 'numeric',
                        month: 'short',
                        day: 'numeric'
                      })}
                      {order.lineItems?.length
                        ? ` • ${order.lineItems.length} ${
                            isRu ? 'товаров' : 'items'
                          }`
                        : ''}
                    </p>
                  </div>
                  <div className='flex items-center gap-4 justify-between sm:justify-end'>
                    <span className='font-semibold text-sm text-foreground'>
                      {order.totalAmount} {order.currencyCode}
                    </span>
                    <Link
                      href={`/${locale}/orders/${order.id}`}
                      className='inline-flex items-center gap-1 rounded-lg border border-border px-2.5 py-1 text-xs font-medium text-muted-foreground hover:text-foreground hover:bg-accent transition'
                    >
                      {isRu ? 'Подробнее' : 'Details'}
                      <ExternalLink className='h-3 w-3' />
                    </Link>
                  </div>
                </div>
              ))}
            </div>
          )}
        </section>
      </div>
    </main>
  );
}
