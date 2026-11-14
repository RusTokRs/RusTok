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

import { ShoppingBag } from "lucide-react";
import { useTranslations } from "@rustok/next-fluent";
import { useCart } from "../context/cart-context";

interface CartTriggerProps {
  className?: string;
  variant?: "floating" | "inline";
  locale?: string;
}

export function CartTrigger({
  className = "",
  variant = "floating",
  locale = "ru",
}: CartTriggerProps) {
  const { openCart, itemCount, isLoading, isUpdating, enabled, actionError } = useCart();
  const messages = useTranslations("Cart");
  const uncertain = !enabled || actionError === "auth" || actionError === "restore" || actionError === "unconfirmed";
  const badge = uncertain ? "—" : isLoading || isUpdating ? "…" : itemCount > 0 ? String(itemCount) : null;
  const badgeLabel = uncertain ? messages("stateUnknown") : isLoading || isUpdating ? messages("loading") : undefined;
  const isRu = locale === "ru";

  if (variant === "inline") {
    return (
      <button
        type="button"
        onClick={openCart}
        disabled={!enabled}
        className={`relative inline-flex h-10 items-center justify-center gap-2 rounded-xl border border-border bg-card px-3 text-xs font-semibold text-foreground shadow-xs hover:bg-accent transition disabled:cursor-not-allowed disabled:opacity-50 ${className}`}
        aria-label={isRu ? "Открыть корзину" : "Open shopping cart"}
      >
        <ShoppingBag className="h-4 w-4 text-primary" />
        <span>{isRu ? "Корзина" : "Cart"}</span>
        {badge !== null && (
          <span role="status" aria-label={badgeLabel} className="flex h-5 min-w-[20px] items-center justify-center rounded-full bg-primary px-1 text-[10px] font-bold text-primary-foreground">
            {badge}
          </span>
        )}
      </button>
    );
  }

  // Floating variant (default: bottom-right corner)
  return (
    <div className={`fixed bottom-6 right-6 z-40 ${className}`}>
      <button
        type="button"
        onClick={openCart}
        disabled={!enabled}
        className={`group relative flex h-14 w-14 items-center justify-center rounded-2xl bg-primary text-primary-foreground shadow-xl transition-all duration-300 hover:scale-105 hover:bg-primary/95 focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 ${
          isUpdating ? "animate-pulse" : ""
        }`}
        aria-label={isRu ? "Открыть корзину покупок" : "Open shopping cart"}
      >
        <ShoppingBag className="h-6 w-6 transition-transform duration-200 group-hover:scale-110" />

        {/* Counter Badge */}
        {badge !== null && (
          <span role="status" aria-label={badgeLabel} className="absolute -top-1.5 -right-1.5 flex h-6 min-w-[24px] items-center justify-center rounded-full border-2 border-background bg-destructive px-1.5 text-xs font-extrabold text-destructive-foreground shadow-md animate-in zoom-in-50 duration-200">
            {!uncertain && !isLoading && !isUpdating && itemCount > 99 ? "99+" : badge}
          </span>
        )}
      </button>
    </div>
  );
}
