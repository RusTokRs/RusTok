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

import { useEffect, useRef } from "react";
import Link from "next/link";
import {
  ArrowRight,
  Loader2,
  Minus,
  Package,
  Plus,
  ShoppingBag,
  Trash2,
  X,
} from "lucide-react";
import { useCart } from "../context/cart-context";

interface CartDrawerProps {
  locale?: string;
}

export function CartDrawer({ locale = "ru" }: CartDrawerProps) {
  const {
    cart,
    isOpen,
    isLoading,
    isUpdating,
    itemCount,
    closeCart,
    updateQuantity,
    removeItem,
  } = useCart();

  const isRu = locale === "ru";
  const drawerRef = useRef<HTMLDivElement>(null);

  // Close on Escape key press
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && isOpen) {
        closeCart();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, closeCart]);

  // Prevent background body scroll when drawer is open
  useEffect(() => {
    if (isOpen) {
      document.body.style.overflow = "hidden";
    } else {
      document.body.style.overflow = "";
    }
    return () => {
      document.body.style.overflow = "";
    };
  }, [isOpen]);

  if (!isOpen) {
    return null;
  }

  const lineItems = cart?.lineItems ?? [];
  const currencyCode = cart?.currencyCode ?? "RUB";

  return (
    <div className="fixed inset-0 z-50 overflow-hidden" role="dialog" aria-modal="true">
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-black/60 backdrop-blur-xs transition-opacity animate-in fade-in duration-300"
        onClick={closeCart}
      />

      {/* Drawer Container */}
      <div className="fixed inset-y-0 right-0 max-w-full flex pl-10">
        <div
          ref={drawerRef}
          className="w-screen max-w-md bg-card border-l border-border shadow-2xl flex flex-col h-full animate-in slide-in-from-right duration-300"
        >
          {/* Header */}
          <div className="flex items-center justify-between border-b border-border px-6 py-4.5 bg-muted/20">
            <div className="flex items-center gap-2.5">
              <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-primary/10 text-primary">
                <ShoppingBag className="h-5 w-5" />
              </div>
              <div>
                <h2 className="text-base font-bold text-foreground">
                  {isRu ? "Корзина покупок" : "Shopping Cart"}
                </h2>
                <p className="text-xs text-muted-foreground">
                  {itemCount > 0
                    ? isRu
                      ? `${itemCount} шт. в заказе`
                      : `${itemCount} items in order`
                    : isRu
                    ? "Пока пусто"
                    : "Currently empty"}
                </p>
              </div>
            </div>

            <button
              type="button"
              onClick={closeCart}
              className="flex h-8 w-8 items-center justify-center rounded-lg text-muted-foreground hover:bg-muted hover:text-foreground transition"
              aria-label="Close cart"
            >
              <X className="h-4 w-4" />
            </button>
          </div>

          {/* Body Content */}
          <div className="flex-1 overflow-y-auto p-6 space-y-4">
            {isLoading && !cart ? (
              <div className="flex h-full flex-col items-center justify-center space-y-3 py-16 text-muted-foreground">
                <Loader2 className="h-8 w-8 animate-spin text-primary" />
                <p className="text-xs font-medium">
                  {isRu ? "Загрузка корзины..." : "Loading cart..."}
                </p>
              </div>
            ) : lineItems.length === 0 ? (
              <div className="flex h-full flex-col items-center justify-center text-center space-y-4 py-16">
                <div className="flex h-16 w-16 items-center justify-center rounded-2xl bg-muted/60 text-muted-foreground">
                  <ShoppingBag className="h-8 w-8 stroke-1" />
                </div>
                <div className="space-y-1">
                  <h3 className="text-base font-semibold text-foreground">
                    {isRu ? "Ваша корзина пуста" : "Your cart is empty"}
                  </h3>
                  <p className="text-xs text-muted-foreground max-w-[240px]">
                    {isRu
                      ? "Выберите интересующие вас товары или наборы в каталоге"
                      : "Explore our catalog to find products and curated bundles"}
                  </p>
                </div>
                <Link
                  href={`/${locale}/products`}
                  onClick={closeCart}
                  className="mt-2 inline-flex items-center gap-2 rounded-xl bg-primary px-5 py-2.5 text-xs font-semibold text-primary-foreground hover:bg-primary/90 transition shadow-xs"
                >
                  {isRu ? "Перейти в каталог" : "Browse Catalog"}
                  <ArrowRight className="h-3.5 w-3.5" />
                </Link>
              </div>
            ) : (
              <div className="space-y-3">
                {lineItems.map((item) => {
                  const unitPriceNum = parseFloat(item.unitPrice) || 0;
                  const totalPriceNum = parseFloat(item.totalPrice) || 0;

                  return (
                    <div
                      key={item.id}
                      className="group relative flex gap-3.5 rounded-2xl border border-border bg-background/50 p-3.5 shadow-xs transition hover:border-border/80"
                    >
                      {/* Product Item Icon / Thumbnail */}
                      <div className="flex h-16 w-16 shrink-0 items-center justify-center rounded-xl bg-muted/60 text-muted-foreground">
                        <Package className="h-8 w-8" />
                      </div>

                      {/* Item Details */}
                      <div className="flex flex-1 flex-col justify-between">
                        <div className="pr-6">
                          <h4 className="text-xs font-semibold text-foreground line-clamp-2">
                            {item.title}
                          </h4>
                          {item.sku && (
                            <p className="mt-0.5 text-[10px] text-muted-foreground font-mono">
                              SKU: {item.sku}
                            </p>
                          )}
                          <p className="mt-1 text-xs font-medium text-muted-foreground">
                            {unitPriceNum.toFixed(2)} {item.currencyCode}
                          </p>
                        </div>

                        {/* Stepper & Line Total */}
                        <div className="mt-2 flex items-center justify-between">
                          <div className="inline-flex h-7 items-center rounded-lg border border-border bg-card px-1">
                            <button
                              type="button"
                              disabled={isUpdating}
                              onClick={() =>
                                void updateQuantity(item.id, item.quantity - 1)
                              }
                              className="flex h-5 w-5 items-center justify-center rounded text-muted-foreground hover:bg-muted hover:text-foreground transition disabled:opacity-50"
                              aria-label="Decrease quantity"
                            >
                              <Minus className="h-3 w-3" />
                            </button>
                            <span className="w-7 text-center text-xs font-semibold text-foreground">
                              {item.quantity}
                            </span>
                            <button
                              type="button"
                              disabled={isUpdating}
                              onClick={() =>
                                void updateQuantity(item.id, item.quantity + 1)
                              }
                              className="flex h-5 w-5 items-center justify-center rounded text-muted-foreground hover:bg-muted hover:text-foreground transition disabled:opacity-50"
                              aria-label="Increase quantity"
                            >
                              <Plus className="h-3 w-3" />
                            </button>
                          </div>

                          <span className="text-xs font-bold text-foreground">
                            {totalPriceNum.toFixed(2)} {item.currencyCode}
                          </span>
                        </div>
                      </div>

                      {/* Remove Button */}
                      <button
                        type="button"
                        disabled={isUpdating}
                        onClick={() => void removeItem(item.id)}
                        className="absolute top-3 right-3 text-muted-foreground hover:text-destructive transition disabled:opacity-50"
                        title={isRu ? "Удалить" : "Remove"}
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </button>
                    </div>
                  );
                })}
              </div>
            )}
          </div>

          {/* Footer with Totals and CTA */}
          {lineItems.length > 0 && cart && (
            <div className="border-t border-border bg-muted/20 p-6 space-y-4">
              <div className="space-y-2 text-xs">
                <div className="flex justify-between text-muted-foreground">
                  <span>{isRu ? "Подитог" : "Subtotal"}</span>
                  <span className="font-medium text-foreground">
                    {parseFloat(cart.subtotalAmount || "0").toFixed(2)}{" "}
                    {currencyCode}
                  </span>
                </div>

                {parseFloat(cart.adjustmentTotal || "0") > 0 && (
                  <div className="flex justify-between text-destructive">
                    <span>{isRu ? "Скидка" : "Discount"}</span>
                    <span>
                      -{parseFloat(cart.adjustmentTotal).toFixed(2)}{" "}
                      {currencyCode}
                    </span>
                  </div>
                )}

                <div className="flex justify-between text-muted-foreground">
                  <span>{isRu ? "Доставка" : "Shipping"}</span>
                  <span className="italic">
                    {parseFloat(cart.shippingTotal || "0") > 0
                      ? `${parseFloat(cart.shippingTotal).toFixed(2)} ${currencyCode}`
                      : isRu
                      ? "Рассчитывается при оформлении"
                      : "Calculated at checkout"}
                  </span>
                </div>

                <div className="flex justify-between border-t border-border pt-2 text-sm font-bold text-foreground">
                  <span>{isRu ? "Итого к оплате" : "Total"}</span>
                  <span className="text-base text-primary">
                    {parseFloat(cart.totalAmount || "0").toFixed(2)}{" "}
                    {currencyCode}
                  </span>
                </div>
              </div>

              {/* Action Buttons */}
              <div className="space-y-2 pt-1">
                <button
                  type="button"
                  onClick={() => {
                    alert(
                      isRu
                        ? "Переход к модулю оформления заказа (Checkout)"
                        : "Proceeding to Checkout module",
                    );
                  }}
                  className="w-full inline-flex h-11 items-center justify-center gap-2 rounded-xl bg-primary text-sm font-semibold text-primary-foreground hover:bg-primary/90 transition shadow-xs"
                >
                  {isRu ? "Оформить заказ" : "Proceed to Checkout"}
                  <ArrowRight className="h-4 w-4" />
                </button>

                <button
                  type="button"
                  onClick={closeCart}
                  className="w-full h-9 inline-flex items-center justify-center rounded-xl text-xs font-semibold text-muted-foreground hover:text-foreground hover:bg-muted/40 transition"
                >
                  {isRu ? "Продолжить покупки" : "Continue Shopping"}
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
