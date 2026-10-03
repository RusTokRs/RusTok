/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

"use client";

import { useTransition } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { Filter, RotateCcw, Search, SlidersHorizontal } from "lucide-react";
import type { ProductCatalogSearchOption } from "../api/types";

interface ProductFiltersProps {
  categoryOptions: ProductCatalogSearchOption[];
  locale: string;
  currentSearch?: string;
  currentCategory?: string;
  currentSortBy?: string;
  currentSortDirection?: string;
}

export function ProductFilters({
  categoryOptions,
  locale,
  currentSearch = "",
  currentCategory = "",
  currentSortBy = "published_at",
  currentSortDirection = "desc",
}: ProductFiltersProps) {
  const router = useRouter();
  const searchParams = useSearchParams();
  const [isPending, startTransition] = useTransition();
  const isRu = locale === "ru";

  const updateParam = (key: string, value: string) => {
    const params = new URLSearchParams(searchParams.toString());
    if (value) {
      params.set(key, value);
    } else {
      params.delete(key);
    }
    // Reset page to 1 when filters change
    if (key !== "page") {
      params.delete("page");
    }
    startTransition(() => {
      router.push(`/${locale}/products?${params.toString()}`);
    });
  };

  const resetAll = () => {
    startTransition(() => {
      router.push(`/${locale}/products`);
    });
  };

  const hasActiveFilters =
    Boolean(currentSearch) ||
    Boolean(currentCategory) ||
    currentSortBy !== "published_at" ||
    currentSortDirection !== "desc";

  return (
    <div className="flex flex-col gap-4 rounded-2xl border border-border bg-card p-4 shadow-xs">
      <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
        {/* Search Input */}
        <div className="relative flex-1 min-w-[200px]">
          <Search className="absolute left-3.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <input
            type="search"
            defaultValue={currentSearch}
            placeholder={
              isRu ? "Поиск по каталогу товаров..." : "Search products in catalog..."
            }
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                updateParam("search", (e.target as HTMLInputElement).value);
              }
            }}
            onBlur={(e) => {
              if (e.target.value !== currentSearch) {
                updateParam("search", e.target.value);
              }
            }}
            className="h-10 w-full rounded-xl border border-border bg-background pl-10 pr-4 text-sm text-foreground placeholder:text-muted-foreground outline-none transition focus:border-primary focus:ring-1 focus:ring-primary"
          />
        </div>

        {/* Controls Container */}
        <div className="flex flex-wrap items-center gap-2.5">
          {/* Category Selector */}
          <div className="relative min-w-[170px]">
            <select
              value={currentCategory}
              onChange={(e) => updateParam("category_id", e.target.value)}
              className="h-10 w-full appearance-none rounded-xl border border-border bg-background px-3.5 pr-8 text-xs font-medium text-foreground outline-none transition focus:border-primary"
            >
              <option value="">
                {isRu ? "Все категории" : "All Categories"}
              </option>
              {categoryOptions.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
            <Filter className="pointer-events-none absolute right-3 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
          </div>

          {/* Sort By Selector */}
          <div className="relative min-w-[170px]">
            <select
              value={`${currentSortBy}:${currentSortDirection}`}
              onChange={(e) => {
                const [by, dir] = e.target.value.split(":");
                const params = new URLSearchParams(searchParams.toString());
                params.set("sort_by", by);
                params.set("sort_direction", dir);
                params.delete("page");
                startTransition(() => {
                  router.push(`/${locale}/products?${params.toString()}`);
                });
              }}
              className="h-10 w-full appearance-none rounded-xl border border-border bg-background px-3.5 pr-8 text-xs font-medium text-foreground outline-none transition focus:border-primary"
            >
              <option value="published_at:desc">
                {isRu ? "Сначала новые" : "Newest First"}
              </option>
              <option value="published_at:asc">
                {isRu ? "Сначала старые" : "Oldest First"}
              </option>
              <option value="created_at:desc">
                {isRu ? "Недавно созданные" : "Recently Created"}
              </option>
            </select>
            <SlidersHorizontal className="pointer-events-none absolute right-3 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
          </div>

          {/* Reset Filters */}
          {hasActiveFilters && (
            <button
              type="button"
              onClick={resetAll}
              disabled={isPending}
              className="inline-flex h-10 items-center gap-1.5 rounded-xl border border-border bg-background px-3 text-xs font-medium text-muted-foreground hover:bg-accent hover:text-foreground transition disabled:opacity-50"
              title={isRu ? "Сбросить фильтры" : "Reset filters"}
            >
              <RotateCcw className="h-3.5 w-3.5" />
              <span className="hidden sm:inline">
                {isRu ? "Сброс" : "Reset"}
              </span>
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
