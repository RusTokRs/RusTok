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
import { getStorefrontTenantSlug } from "@/shared/api/modules";
import { buildSeoMetadata } from "@/shared/seo/metadata";
import { resolveSeoPageContextForRoute } from "@/shared/seo/runtime";
import { CheckoutView } from "@rustok/cart-frontend";

interface CheckoutPageProps {
  params: Promise<{ locale: string }>;
}

export async function generateMetadata({
  params,
}: CheckoutPageProps): Promise<Metadata> {
  const { locale } = await params;
  const isRu = locale === "ru";
  const path = "/checkout";
  const seoResolution = await resolveSeoPageContextForRoute({
    locale,
    route: path,
  });

  return buildSeoMetadata({
    locale,
    title: isRu ? "Оформление заказа | RusToK" : "Checkout | RusToK",
    description: isRu
      ? "Оформление заказа с выбором адреса доставки, способа доставки и оплаты"
      : "Secure checkout flow with shipping address, delivery options, and payment selection",
    path,
    context: seoResolution.context,
  });
}

export default async function CheckoutPage({ params }: CheckoutPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);
  const tenantSlug = getStorefrontTenantSlug();

  return (
    <main className="min-h-screen bg-background">
      <CheckoutView locale={locale} tenantSlug={tenantSlug} />
    </main>
  );
}
