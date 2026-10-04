/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import type { Metadata } from "next";
import { cookies } from "next/headers";
import { setRequestLocale } from "@rustok/next-fluent/server";
import { storefrontGraphql } from "@/shared/lib/graphql";
import { getStorefrontTenantSlug } from "@/shared/api/modules";
import { buildSeoMetadata } from "@/shared/seo/metadata";
import { resolveSeoPageContextForRoute } from "@/shared/seo/runtime";
import { fetchStorefrontOrders, OrdersHistoryView } from "@rustok/order-frontend";
import { ADMIN_TOKEN_KEY } from "@rustok/ui-auth/browser";

interface OrdersIndexPageProps {
  params: Promise<{ locale: string }>;
}

export async function generateMetadata({
  params,
}: OrdersIndexPageProps): Promise<Metadata> {
  const { locale } = await params;
  const isRu = locale.toLowerCase().startsWith("ru");
  const path = "/orders";
  const seoResolution = await resolveSeoPageContextForRoute({
    locale,
    route: path,
  });

  return buildSeoMetadata({
    locale,
    title: isRu ? "Заказы | RusToK" : "Orders | RusToK",
    description: isRu
      ? "Просмотр ваших заказов, отслеживание доставки и поиск по номеру"
      : "View your orders, track shipments, and lookup order status",
    path,
    context: seoResolution.context,
  });
}

export default async function OrdersIndexPage({
  params,
}: OrdersIndexPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);
  const tenantSlug = getStorefrontTenantSlug();

  const cookieStore = await cookies();
  const token = cookieStore.get(ADMIN_TOKEN_KEY)?.value ?? null;

  const ordersData = token
    ? await fetchStorefrontOrders(storefrontGraphql, { perPage: 25 }, tenantSlug, token)
    : null;

  return (
    <main className="min-h-screen bg-background">
      <OrdersHistoryView
        initialOrders={ordersData?.items ?? []}
        locale={locale}
        tenantSlug={tenantSlug}
        authToken={token}
      />
    </main>
  );
}
