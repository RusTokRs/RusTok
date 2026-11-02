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

import React, { useEffect, useMemo, useState } from "react";
import type { storefrontGraphql } from "@/shared/lib/graphql";
import { useTranslations } from "@rustok/next-fluent";
import { Button } from "@/shared/ui/base/button";
import type {
  PricingResolutionContext,
  StorefrontPricingData,
  StorefrontPricingQuery,
} from "../types";
import { fetchStorefrontPricing } from "../api";
import {
  createPricingSelection,
  pricingContextQuery,
  pricingCurrencies,
  pricingDiscovery,
  pricingReadKey,
  pricingSelectionInScope,
  updatePricingContext,
  visiblePricingValue,
} from "../core";
import { PricingContextBar } from "./pricing-context-bar";
import { PricingProductCard } from "./pricing-product-card";
import { PricingProductDetailView } from "./pricing-product-detail";

type GraphqlExecutor = typeof storefrontGraphql;

export interface PricingViewProps {
  initialData?: StorefrontPricingData | null;
  initialQuery?: StorefrontPricingQuery;
  resetHref: string;
  graphql: GraphqlExecutor;
  token?: string | null;
  tenantSlug?: string | null;
  locale?: string | null;
}

export function PricingView({
  initialData = null,
  initialQuery,
  resetHref,
  graphql,
  token,
  tenantSlug,
  locale = "en",
}: PricingViewProps): React.JSX.Element {
  const scope = useMemo(
    () => ({
      key: pricingReadKey({ locale }, tenantSlug, token),
      executor: graphql,
    }),
    [locale, tenantSlug, token, graphql],
  );
  const [selectionState, setSelection] = useState(() =>
    createPricingSelection(scope, initialData, initialQuery),
  );
  const selection = useMemo(
    () => pricingSelectionInScope(selectionState, scope),
    [selectionState, scope],
  );
  const { context, selectedHandle } = selection;
  const query = { ...pricingContextQuery(context), selectedHandle, locale };
  const readKey = pricingReadKey(query, tenantSlug, token);
  const discoveryKey = pricingReadKey(
    { locale, channelId: context.channelId, channelSlug: context.channelSlug },
    tenantSlug,
    token,
  );
  const [result, setResult] = useState({
    scope,
    key: readKey,
    value: initialData,
  });
  const data = visiblePricingValue(result, scope, readKey);
  const [discoveryResult, setDiscovery] = useState(() => ({
    scope,
    key: discoveryKey,
    value: pricingDiscovery(initialData),
  }));
  // Channel discovery is tenant-scoped; lists and currency coverage are channel-scoped.
  // Only these non-quote controls survive a failed re-quote so the user can correct it.
  const scopeDiscovery = visiblePricingValue(discoveryResult, scope);
  const channelDiscovery = visiblePricingValue(
    discoveryResult,
    scope,
    discoveryKey,
  );
  const [loading, setLoading] = useState(!initialData);
  const [failure, setFailure] = useState<{
    scope: typeof scope;
    key: string;
    value: string | null;
  }>({ scope, key: readKey, value: null });
  const error = visiblePricingValue(failure, scope, readKey);

  const pricingMessages = useTranslations("Pricing");
  const isRu = locale === "ru";
  const badge = isRu ? "Ценообразование" : "Pricing";
  const title = isRu
    ? "Публичный атлас цен RusToK"
    : "Public pricing atlas from the pricing module";
  const subtitle = isRu
    ? "Этот раздел витрины отображает видимость цен, валютное покрытие и распродажи напрямую через модуль pricing."
    : "This storefront route reads pricing visibility, currency coverage and sale markers through the pricing-owned package.";

  const handleContextChange = (updated: Partial<PricingResolutionContext>) => {
    setSelection((prev) => {
      const current = pricingSelectionInScope(prev, scope);
      return {
        ...current,
        context: updatePricingContext(
          current.context,
          updated,
          scopeDiscovery?.availableChannels ?? [],
        ),
      };
    });
  };
  const handleProductSelection = (handle: string | null) => {
    setSelection((prev) => ({
      ...pricingSelectionInScope(prev, scope),
      selectedHandle: handle,
    }));
  };

  useEffect(() => {
    let cancelled = false;
    // Commit the already-derived scope without overwriting newer same-scope input.
    // Returning to an earlier scope must not resurrect its dormant selection state.
    setSelection((current) =>
      pricingSelectionInScope(current, scope) === current ? current : selection,
    );
    setLoading(true);
    setFailure({ scope, key: readKey, value: null });

    fetchStorefrontPricing(
      graphql,
      { ...pricingContextQuery(context), selectedHandle, locale },
      token,
      tenantSlug,
    )
      .then((res) => {
        if (!cancelled) {
          setResult({ scope, key: readKey, value: res });
          setDiscovery({
            scope,
            key: discoveryKey,
            value: pricingDiscovery(res),
          });
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setResult({ scope, key: readKey, value: null });
          setFailure({
            scope,
            key: readKey,
            value:
              err?.message ??
              (isRu
                ? "Не удалось загрузить данные ценообразования"
                : "Failed to load pricing data"),
          });
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [
    graphql,
    selectedHandle,
    context,
    locale,
    token,
    tenantSlug,
    isRu,
    readKey,
    discoveryKey,
    scope,
    selection,
  ]);

  return (
    <section className="space-y-8">
      <header className="rounded-[2rem] border border-border bg-card p-6 shadow-sm sm:p-8">
        <span className="inline-flex rounded-full border border-border px-3 py-1 text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground">
          {badge}
        </span>
        <h1 className="mt-3 text-2xl font-bold text-card-foreground sm:text-3xl">
          {title}
        </h1>
        <p className="mt-2 max-w-2xl text-sm leading-6 text-muted-foreground">
          {subtitle}
        </p>

        <div className="mt-6">
          <PricingContextBar
            availableCurrencies={pricingCurrencies(
              channelDiscovery,
              context.currencyCode,
            )}
            availableChannels={scopeDiscovery?.availableChannels ?? []}
            activePriceLists={channelDiscovery?.activePriceLists ?? []}
            context={context}
            onChange={handleContextChange}
            locale={locale}
          />
        </div>
        <Button asChild variant="outline" size="sm" className="mt-4">
          <a href={resetHref}>{pricingMessages("resetContext")}</a>
        </Button>
      </header>

      {error ? (
        <div
          role="alert"
          className="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive"
        >
          {error}
        </div>
      ) : null}

      {loading && !data ? (
        <div className="space-y-4">
          <div className="h-48 animate-pulse rounded-3xl bg-muted" />
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            <div className="h-32 animate-pulse rounded-2xl bg-muted" />
            <div className="h-32 animate-pulse rounded-2xl bg-muted" />
            <div className="h-32 animate-pulse rounded-2xl bg-muted" />
          </div>
        </div>
      ) : data ? (
        <div className="space-y-8">
          {data.selectedProduct && (
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <h2 className="text-lg font-semibold text-card-foreground">
                  {isRu
                    ? "Детализация выбранного товара"
                    : "Selected Product Detail"}
                </h2>
                <button
                  type="button"
                  onClick={() => handleProductSelection(null)}
                  className="text-xs font-medium text-primary hover:underline"
                >
                  {isRu ? "✕ Сбросить выбор" : "✕ Clear selection"}
                </button>
              </div>
              <PricingProductDetailView
                product={data.selectedProduct}
                locale={locale}
              />
            </div>
          )}

          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <h2 className="text-lg font-semibold text-card-foreground">
                {isRu ? "Каталог товаров с ценами" : "Product Pricing Catalog"}
              </h2>
              <span className="text-xs text-muted-foreground">
                {isRu
                  ? `Всего товаров: ${data.products.total}`
                  : `Total products: ${data.products.total}`}
              </span>
            </div>

            {data.products.items.length === 0 ? (
              <div className="rounded-2xl border border-dashed border-border bg-card p-10 text-center text-sm text-muted-foreground">
                {isRu ? "Товары не найдены" : "No products found"}
              </div>
            ) : (
              <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
                {data.products.items.map((prod) => (
                  <PricingProductCard
                    key={prod.id}
                    product={prod}
                    isSelected={selectedHandle === prod.handle}
                    onSelect={(handle) =>
                      handleProductSelection(
                        selectedHandle === handle ? null : handle,
                      )
                    }
                    locale={locale}
                  />
                ))}
              </div>
            )}
          </div>
        </div>
      ) : null}
    </section>
  );
}
