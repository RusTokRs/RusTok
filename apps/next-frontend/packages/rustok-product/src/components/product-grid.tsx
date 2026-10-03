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
import { ChevronLeft, ChevronRight, PackageOpen } from "lucide-react";
import type {
  ProductCatalogSearchOption,
  StorefrontProductListItem,
} from "../api/types";
import { ProductCard } from "./product-card";
import { ProductFilters } from "./product-filters";

interface ProductGridProps {
  products: StorefrontProductListItem[];
  total: number;
  currentPage: number;
  perPage: number;
  hasNext: boolean;
  locale: string;
  categoryOptions?: ProductCatalogSearchOption[];
  currentSearch?: string;
  currentCategory?: string;
  currentSortBy?: string;
  currentSortDirection?: string;
  showFilters?: boolean;
}

export function ProductGrid({
  products,
  total,
  currentPage,
  perPage,
  hasNext,
  locale,
  categoryOptions = [],
  currentSearch = "",
  currentCategory = "",
  currentSortBy = "published_at",
  currentSortDirection = "desc",
  showFilters = true,
}: ProductGridProps) {
  const isRu = locale === "ru";
  const totalPages = Math.max(1, Math.ceil(total / perPage));

  const buildPageUrl = (page: number) => {
    const params = new URLSearchParams();
    if (currentSearch) params.set("search", currentSearch);
    if (currentCategory) params.set("category_id", currentCategory);
    if (currentSortBy !== "published_at") params.set("sort_by", currentSortBy);
    if (currentSortDirection !== "desc")
      params.set("sort_direction", currentSortDirection);
    if (page > 1) params.set("page", page.toString());
    const query = params.toString();
    return `/${locale}/products${query ? `?${query}` : ""}`;
  };

  return (
    <div className="space-y-6">
      {/* Optional Filters Bar */}
      {showFilters && (
        <ProductFilters
          categoryOptions={categoryOptions}
          locale={locale}
          currentSearch={currentSearch}
          currentCategory={currentCategory}
          currentSortBy={currentSortBy}
          currentSortDirection={currentSortDirection}
        />
      )}

      {/* Grid or Empty State */}
      {products.length === 0 ? (
        <div className="flex flex-col items-center justify-center rounded-2xl border border-dashed border-border bg-card/40 py-16 text-center">
          <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-muted/60 text-muted-foreground">
            <PackageOpen className="h-7 w-7" />
          </div>
          <h3 className="mt-4 text-base font-semibold text-foreground">
            {isRu ? "Товары не найдены" : "No products found"}
          </h3>
          <p className="mt-1 max-w-sm text-xs text-muted-foreground">
            {isRu
              ? "Попробуйте изменить параметры поиска или сбросить активные фильтры."
              : "Try adjusting your search query or clear the active category filters."}
          </p>
          {(currentSearch || currentCategory) && (
            <Link
              href={`/${locale}/products`}
              className="mt-4 inline-flex h-9 items-center justify-center rounded-xl bg-primary px-4 text-xs font-semibold text-primary-foreground shadow-xs hover:bg-primary/90 transition"
            >
              {isRu ? "Сбросить фильтры" : "Clear filters"}
            </Link>
          )}
        </div>
      ) : (
        <>
          <div className="grid grid-cols-1 gap-5 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
            {products.map((product) => (
              <ProductCard
                key={product.id}
                product={product}
                locale={locale}
              />
            ))}
          </div>

          {/* Pagination Controls */}
          {totalPages > 1 && (
            <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4 border-t border-border pt-6">
              <div className="text-xs text-muted-foreground">
                {isRu
                  ? `Показано ${(currentPage - 1) * perPage + 1}—${Math.min(
                      currentPage * perPage,
                      total,
                    )} из ${total} товаров`
                  : `Showing ${(currentPage - 1) * perPage + 1}—${Math.min(
                      currentPage * perPage,
                      total,
                    )} of ${total} products`}
              </div>

              <div className="flex items-center gap-2">
                <Link
                  href={buildPageUrl(Math.max(1, currentPage - 1))}
                  className={`inline-flex h-9 w-9 items-center justify-center rounded-xl border border-border bg-card text-foreground transition hover:bg-accent ${
                    currentPage <= 1
                      ? "pointer-events-none opacity-40"
                      : ""
                  }`}
                  aria-label="Previous page"
                >
                  <ChevronLeft className="h-4 w-4" />
                </Link>

                <span className="px-3 text-xs font-medium text-foreground">
                  {currentPage} / {totalPages}
                </span>

                <Link
                  href={buildPageUrl(currentPage + 1)}
                  className={`inline-flex h-9 w-9 items-center justify-center rounded-xl border border-border bg-card text-foreground transition hover:bg-accent ${
                    !hasNext || currentPage >= totalPages
                      ? "pointer-events-none opacity-40"
                      : ""
                  }`}
                  aria-label="Next page"
                >
                  <ChevronRight className="h-4 w-4" />
                </Link>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}
