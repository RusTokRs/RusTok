/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import Link from "next/link";
import { ArrowRight, Sparkles } from "lucide-react";
import { storefrontGraphql } from "@/shared/lib/graphql";
import {
  fetchCatalogSearchOptions,
  fetchStorefrontProducts,
} from "../api/products";
import { ProductCard } from "./product-card";

interface CatalogSectionProps {
  locale: string;
  tenantSlug?: string | null;
  searchParams?: Record<string, string | string[] | undefined>;
}

export async function CatalogSection({
  locale,
  tenantSlug,
  searchParams = {},
}: CatalogSectionProps) {
  const isRu = locale === "ru";
  const categoryId =
    typeof searchParams.category_id === "string"
      ? searchParams.category_id
      : undefined;

  let productsResult;
  let options;
  try {
    [productsResult, options] = await Promise.all([
      fetchStorefrontProducts(
        storefrontGraphql,
        locale,
        {
          categoryId,
          page: 1,
          perPage: 8,
          sortBy: "published_at",
          sortDirection: "desc",
        },
        tenantSlug
      ),
      fetchCatalogSearchOptions(storefrontGraphql, locale, tenantSlug),
    ]);
  } catch {
    return null;
  }

  const products = productsResult.items;
  if (products.length === 0) return null;

  return (
    <section className="space-y-6">
      {/* Section Header */}
      <div className="flex flex-col sm:flex-row sm:items-end justify-between gap-4 border-b border-border pb-4">
        <div>
          <div className="flex items-center gap-2">
            <span className="inline-flex items-center gap-1 rounded-full bg-primary/10 px-2.5 py-0.5 text-xs font-semibold text-primary">
              <Sparkles className="h-3 w-3" />
              {isRu ? "Каталог" : "Catalog"}
            </span>
          </div>
          <h2 className="mt-2 text-2xl font-bold tracking-tight text-foreground sm:text-3xl">
            {isRu ? "Популярные товары" : "Featured Products"}
          </h2>
          <p className="mt-1 text-xs sm:text-sm text-muted-foreground">
            {isRu
              ? "Ознакомьтесь с актуальными товарами и наборами нашего магазина"
              : "Discover handpicked products, variants and curated bundles"}
          </p>
        </div>

        <Link
          href={`/${locale}/products`}
          className="inline-flex items-center gap-1 text-xs font-semibold text-primary hover:underline group"
        >
          {isRu ? "Смотреть весь каталог" : "View full catalog"}
          <ArrowRight className="h-3.5 w-3.5 transition-transform group-hover:translate-x-0.5" />
        </Link>
      </div>

      {/* Category Pills (if any) */}
      {options.categoryOptions.length > 0 && (
        <div className="flex items-center gap-2 overflow-x-auto pb-2 scrollbar-none">
          <Link
            href={`/${locale}`}
            className={`rounded-full px-3.5 py-1 text-xs font-medium transition ${
              !categoryId
                ? "bg-primary text-primary-foreground shadow-xs"
                : "border border-border bg-card text-muted-foreground hover:bg-accent hover:text-foreground"
            }`}
          >
            {isRu ? "Все товары" : "All Products"}
          </Link>
          {options.categoryOptions.map((cat) => (
            <Link
              key={cat.value}
              href={`/${locale}?category_id=${cat.value}`}
              className={`whitespace-nowrap rounded-full px-3.5 py-1 text-xs font-medium transition ${
                categoryId === cat.value
                  ? "bg-primary text-primary-foreground shadow-xs"
                  : "border border-border bg-card text-muted-foreground hover:bg-accent hover:text-foreground"
              }`}
            >
              {cat.label}
            </Link>
          ))}
        </div>
      )}

      {/* Products Grid */}
      <div className="grid grid-cols-1 gap-5 sm:grid-cols-2 lg:grid-cols-4">
        {products.map((product) => (
          <ProductCard key={product.id} product={product} locale={locale} />
        ))}
      </div>
    </section>
  );
}
