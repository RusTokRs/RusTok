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
import { notFound } from "next/navigation";
import { storefrontGraphql } from "@/shared/lib/graphql";
import { getStorefrontTenantSlug } from "@/shared/api/modules";
import { buildSeoMetadata } from "@/shared/seo/metadata";
import { resolveSeoPageContextForRoute } from "@/shared/seo/runtime";
import {
  fetchStorefrontProduct,
  fetchStorefrontProductPricing,
  ProductDetailView,
} from "../../../../../packages/rustok-product/src";

interface ProductPageProps {
  params: Promise<{ locale: string; slug: string }>;
}

export async function generateMetadata({
  params,
}: ProductPageProps): Promise<Metadata> {
  const { locale, slug } = await params;
  const tenantSlug = getStorefrontTenantSlug();
  const product = await fetchStorefrontProduct(
    storefrontGraphql,
    slug,
    locale,
    tenantSlug
  );

  const path = `/products/${slug}`;
  const seoResolution = await resolveSeoPageContextForRoute({
    locale,
    route: path,
  });

  const translation =
    product?.translations.find((t) => t.locale === locale) ||
    product?.translations[0];

  const title = translation?.title || product?.handle || "Product Details";
  const description =
    translation?.description?.slice(0, 160) ||
    "Buy product online at RusToK Storefront.";

  return buildSeoMetadata({
    locale,
    title: `${title} | RusToK`,
    description,
    path,
    context: seoResolution.context,
  });
}

export default async function ProductDetailPage({ params }: ProductPageProps) {
  const { locale, slug } = await params;
  const tenantSlug = getStorefrontTenantSlug();

  const product = await fetchStorefrontProduct(
    storefrontGraphql,
    slug,
    locale,
    tenantSlug
  );

  if (!product) {
    notFound();
  }

  const pricingData = await fetchStorefrontProductPricing(
    storefrontGraphql,
    slug,
    locale,
    undefined,
    tenantSlug
  );

  return (
    <main className="min-h-screen bg-background">
      <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8 py-10">
        <ProductDetailView
          product={product}
          pricingVariants={pricingData?.variants || []}
          locale={locale}
        />
      </div>
    </main>
  );
}
