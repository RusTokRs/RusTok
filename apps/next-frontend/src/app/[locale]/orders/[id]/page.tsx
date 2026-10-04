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
import { setRequestLocale } from "@rustok/next-fluent/server";
import { storefrontGraphql } from "@/shared/lib/graphql";
import { getStorefrontTenantSlug } from "@/shared/api/modules";
import { buildSeoMetadata } from "@/shared/seo/metadata";
import { resolveSeoPageContextForRoute } from "@/shared/seo/runtime";
import { fetchStorefrontOrder, OrderView } from "@rustok/order-frontend";

interface OrderPageProps {
  params: Promise<{ locale: string; id: string }>;
}

export async function generateMetadata({
  params,
}: OrderPageProps): Promise<Metadata> {
  const { locale, id } = await params;
  const isRu = locale.toLowerCase().startsWith("ru");
  const path = `/orders/${id}`;
  const seoResolution = await resolveSeoPageContextForRoute({
    locale,
    route: path,
  });

  const displayId = id.length > 8 ? id.slice(0, 8) : id;

  return buildSeoMetadata({
    locale,
    title: isRu ? `Заказ #${displayId} | RusToK` : `Order #${displayId} | RusToK`,
    description: isRu
      ? `Просмотр статуса, состава и информации о доставке заказа #${displayId}`
      : `Track order #${displayId} status, items breakdown, and delivery updates`,
    path,
    context: seoResolution.context,
  });
}

export default async function OrderDetailPage({ params }: OrderPageProps) {
  const { locale, id } = await params;
  setRequestLocale(locale);
  const tenantSlug = getStorefrontTenantSlug();

  const order = await fetchStorefrontOrder(storefrontGraphql, id, tenantSlug);

  return (
    <main className="min-h-screen bg-background">
      <OrderView order={order} orderId={id} locale={locale} />
    </main>
  );
}
