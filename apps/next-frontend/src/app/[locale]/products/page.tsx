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
import { getTranslations } from "@rustok/next-fluent/server";
import { storefrontGraphql } from "@/shared/lib/graphql";
import { getStorefrontTenantSlug } from "@/shared/api/modules";
import { buildSeoMetadata } from "@/shared/seo/metadata";
import { resolveSeoPageContextForRoute } from "@/shared/seo/runtime";
import {
  buildCatalogFacetCodes,
  fetchCatalogSearchOptions,
  fetchStorefrontCatalogFacets,
  fetchStorefrontProducts,
  parseAttributeFilters,
  ProductGrid,
} from "../../../../packages/rustok-product/src";
import type { ProductCatalogFacet } from "../../../../packages/rustok-product/src";

interface ProductsPageProps {
  params: Promise<{ locale: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}

export async function generateMetadata({
  params,
}: {
  params: Promise<{ locale: string }>;
}): Promise<Metadata> {
  const { locale } = await params;
  const isRu = locale === "ru";
  const path = "/products";
  const seoResolution = await resolveSeoPageContextForRoute({
    locale,
    route: path,
  });

  return buildSeoMetadata({
    locale,
    title: isRu ? "Каталог товаров | RusToK" : "Product Catalog | RusToK",
    description: isRu
      ? "Исследуйте наш каталог товаров, комплектов и вариантов с быстрой доставкой"
      : "Explore our collection of products, variants, and curated bundles with fast delivery",
    path,
    context: seoResolution.context,
  });
}

export default async function ProductsCatalogPage({
  params,
  searchParams,
}: ProductsPageProps) {
  const { locale } = await params;
  const query = await searchParams;
  const isRu = locale === "ru";
  const tenantSlug = getStorefrontTenantSlug();

  const search = typeof query.search === "string" ? query.search : undefined;
  const categoryId =
    typeof query.category_id === "string" ? query.category_id : undefined;
  const sortBy =
    typeof query.sort_by === "string" ? query.sort_by : "published_at";
  const sortDirection =
    typeof query.sort_direction === "string" ? query.sort_direction : "desc";
  const page =
    typeof query.page === "string" ? Math.max(1, parseInt(query.page, 10) || 1) : 1;
  const perPage = 12;
  const currencyCode =
    typeof query.currency === "string"
      ? query.currency.trim().toUpperCase()
      : undefined;
  const attributeFilters = parseAttributeFilters(
    typeof query.attribute_filters === "string"
      ? query.attribute_filters
      : undefined
  );

  let productsResult;
  let searchOptions;

  try {
    [productsResult, searchOptions] = await Promise.all([
      fetchStorefrontProducts(
        storefrontGraphql,
        locale,
        {
          search,
          categoryId,
          sortBy,
          sortDirection,
          currencyCode,
          attributeFilters:
            attributeFilters.length > 0 ? attributeFilters : undefined,
          page,
          perPage,
        },
        tenantSlug
      ),
      fetchCatalogSearchOptions(storefrontGraphql, locale, tenantSlug),
    ]);
  } catch {
    productsResult = {
      total: 0,
      page: 1,
      perPage,
      hasNext: false,
      items: [],
    };
    searchOptions = {
      categoryOptions: [],
      attributeOptions: [],
    };
  }

  // Facets describe the whole filtered catalog, so they are requested once per page render with
  // the same controls the grid uses — minus pagination. Counting is optional: an owner without the
  // facet capability must not take the catalog page down with it.
  let facets: ProductCatalogFacet[] = [];
  try {
    facets = await fetchStorefrontCatalogFacets(
      storefrontGraphql,
      locale,
      {
        search,
        categoryId,
        sortBy,
        sortDirection,
        currencyCode,
        attributeFilters:
          attributeFilters.length > 0 ? attributeFilters : undefined,
      },
      buildCatalogFacetCodes(searchOptions),
      tenantSlug
    );
  } catch {
    facets = [];
  }

  return (
    <main className="min-h-screen bg-background">
      <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8 py-10 space-y-8">
        {/* Page Header */}
        <div className="space-y-2 border-b border-border pb-6">
          <h1 className="text-3xl font-bold tracking-tight text-foreground sm:text-4xl">
            {isRu ? "Каталог товаров" : "Product Catalog"}
          </h1>
          <p className="text-sm text-muted-foreground max-w-2xl">
            {isRu
              ? "Ознакомьтесь с актуальным ассортиментом, подберите нужные варианты и оформите заказ в несколько кликов."
              : "Browse our current inventory, select tailored product options, and place your order in seconds."}
          </p>
        </div>

        {/* Product Grid & Filters */}
        <ProductGrid
          products={productsResult.items}
          total={productsResult.total}
          currentPage={productsResult.page}
          perPage={productsResult.perPage}
          hasNext={productsResult.hasNext}
          locale={locale}
          categoryOptions={searchOptions.categoryOptions}
          currentSearch={search}
          currentCategory={categoryId}
          currentSortBy={sortBy}
          currentSortDirection={sortDirection}
          currentAttributeFilters={attributeFilters}
          facets={facets}
          currencyCode={currencyCode}
        />
      </div>
    </main>
  );
}
