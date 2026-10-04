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

import { useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import {
  ArrowRight,
  Calendar,
  CheckCircle2,
  Clock,
  CreditCard,
  Package,
  Search,
  ShoppingBag,
  Truck,
  XCircle,
} from "lucide-react";
import { storefrontGraphql } from "@/shared/lib/graphql";
import {
  fetchGuestRecentOrders,
  fetchStorefrontOrders,
  getLocalRecentOrderIds,
} from "../api/order";
import type { Order } from "../api/types";

export interface OrdersHistoryViewProps {
  initialOrders?: Order[];
  locale?: string;
  tenantSlug?: string | null;
  authToken?: string | null;
}

type TabType = "all" | "active" | "delivered" | "cancelled" | "lookup";

export function OrdersHistoryView({
  initialOrders = [],
  locale = "ru",
  tenantSlug = null,
  authToken = null,
}: OrdersHistoryViewProps) {
  const router = useRouter();
  const isRu = locale.toLowerCase().startsWith("ru");

  const [orders, setOrders] = useState<Order[]>(initialOrders);
  const [isLoading, setIsLoading] = useState<boolean>(initialOrders.length === 0);
  const [activeTab, setActiveTab] = useState<TabType>("all");
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [lookupId, setLookupId] = useState<string>("");
  const [lookupError, setLookupError] = useState<string | null>(null);

  // Fetch orders from server (if authenticated) or local storage (if guest)
  useEffect(() => {
    let isCancelled = false;

    async function loadOrders() {
      setIsLoading(true);
      try {
        if (authToken) {
          // 1. Authenticated customer: fetch from GraphQL
          const response = await fetchStorefrontOrders(
            storefrontGraphql,
            { perPage: 50 },
            tenantSlug,
            authToken,
          );
          if (!isCancelled && response?.items) {
            setOrders(response.items);
            return;
          }
        }

        // 2. Guest fallback: load from local stored IDs
        const localIds = getLocalRecentOrderIds();
        if (localIds.length > 0) {
          const guestOrders = await fetchGuestRecentOrders(
            storefrontGraphql,
            localIds,
            tenantSlug,
          );
          if (!isCancelled) {
            setOrders(guestOrders);
          }
        } else if (!isCancelled) {
          setOrders(initialOrders);
        }
      } catch (err) {
        console.error("Failed to load customer orders:", err);
      } finally {
        if (!isCancelled) {
          setIsLoading(false);
        }
      }
    }

    void loadOrders();

    return () => {
      isCancelled = true;
    };
  }, [authToken, tenantSlug, initialOrders]);

  // Filter orders based on active tab and search query
  const filteredOrders = useMemo(() => {
    return orders.filter((order) => {
      const status = (order.status || "").toUpperCase();

      // Tab filter
      if (activeTab === "active") {
        if (status === "DELIVERED" || status === "CANCELLED") return false;
      } else if (activeTab === "delivered") {
        if (status !== "DELIVERED") return false;
      } else if (activeTab === "cancelled") {
        if (status !== "CANCELLED") return false;
      }

      // Query filter
      if (searchQuery.trim()) {
        const query = searchQuery.trim().toLowerCase();
        const matchesId = order.id.toLowerCase().includes(query);
        const matchesTracking = order.trackingNumber?.toLowerCase().includes(query);
        const matchesItems = order.lineItems.some((item) =>
          item.title.toLowerCase().includes(query),
        );
        return matchesId || matchesTracking || matchesItems;
      }

      return true;
    });
  }, [orders, activeTab, searchQuery]);

  const handleLookupSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const clean = lookupId.trim();
    if (!clean) {
      setLookupError(
        isRu ? "Пожалуйста, введите номер заказа" : "Please enter an order ID",
      );
      return;
    }
    setLookupError(null);
    router.push(`/${locale}/orders/${clean}`);
  };

  const getStatusBadge = (rawStatus: string) => {
    const status = rawStatus.toUpperCase();
    switch (status) {
      case "DELIVERED":
        return {
          label: isRu ? "Доставлен" : "Delivered",
          icon: CheckCircle2,
          className: "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400",
        };
      case "SHIPPED":
        return {
          label: isRu ? "В пути" : "In Transit",
          icon: Truck,
          className: "bg-sky-500/10 text-sky-600 dark:text-sky-400",
        };
      case "PROCESSING":
        return {
          label: isRu ? "Сборка" : "Processing",
          icon: Package,
          className: "bg-amber-500/10 text-amber-600 dark:text-amber-400",
        };
      case "CONFIRMED":
        return {
          label: isRu ? "Оплачен" : "Paid",
          icon: CreditCard,
          className: "bg-indigo-500/10 text-indigo-600 dark:text-indigo-400",
        };
      case "CANCELLED":
        return {
          label: isRu ? "Отменен" : "Cancelled",
          icon: XCircle,
          className: "bg-destructive/10 text-destructive",
        };
      case "PENDING":
      default:
        return {
          label: isRu ? "Оформлен" : "Order Placed",
          icon: Clock,
          className: "bg-primary/10 text-primary",
        };
    }
  };

  return (
    <div className="mx-auto max-w-7xl px-4 py-10 sm:px-6 lg:px-8">
      {/* Top Header */}
      <div className="mb-8 flex flex-col md:flex-row md:items-end justify-between gap-4 border-b border-border pb-6">
        <div>
          <div className="flex items-center gap-2 text-xs font-bold uppercase tracking-wider text-primary">
            <ShoppingBag className="h-4 w-4" />
            <span>{isRu ? "Личный кабинет" : "Customer Portal"}</span>
          </div>
          <h1 className="mt-2 text-2xl font-extrabold tracking-tight text-foreground sm:text-3xl">
            {isRu ? "История заказов" : "Order History"}
          </h1>
          <p className="mt-1 text-sm text-muted-foreground">
            {isRu
              ? "Отслеживайте статус активных доставок и просматривайте историю предыдущих покупок"
              : "Track active shipments, verify delivery status, and review past orders"}
          </p>
        </div>

        {/* Quick Search in list */}
        <div className="w-full md:w-72">
          <div className="relative">
            <Search className="absolute left-3.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder={
                isRu ? "Поиск по номеру или товару..." : "Search orders or items..."
              }
              className="h-10 w-full rounded-xl border border-border bg-card pl-10 pr-4 text-xs font-medium text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary shadow-xs"
            />
          </div>
        </div>
      </div>

      {/* Tabs Row */}
      <div className="mb-8 flex flex-wrap items-center gap-2 border-b border-border/60 pb-3">
        {(
          [
            { id: "all", label: isRu ? "Все заказы" : "All Orders" },
            { id: "active", label: isRu ? "В пути / Сборка" : "In Progress" },
            { id: "delivered", label: isRu ? "Доставленные" : "Delivered" },
            { id: "cancelled", label: isRu ? "Отмененные" : "Cancelled" },
            { id: "lookup", label: isRu ? "Поиск по номеру" : "Lookup by ID" },
          ] as const
        ).map((tab) => {
          const isActive = activeTab === tab.id;
          return (
            <button
              key={tab.id}
              type="button"
              onClick={() => setActiveTab(tab.id)}
              className={`rounded-xl px-4 py-2 text-xs font-semibold transition cursor-pointer ${
                isActive
                  ? "bg-primary text-primary-foreground shadow-xs"
                  : "bg-card text-muted-foreground hover:bg-muted hover:text-foreground border border-border"
              }`}
            >
              {tab.label}
            </button>
          );
        })}
      </div>

      {/* Lookup Tab Panel */}
      {activeTab === "lookup" ? (
        <div className="mx-auto max-w-xl rounded-3xl border border-border bg-card p-8 sm:p-10 shadow-xs text-center">
          <div className="mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-2xl bg-primary/10 text-primary">
            <Search className="h-6 w-6" />
          </div>
          <h2 className="text-xl font-bold text-foreground">
            {isRu ? "Отследить заказ по номеру" : "Track Order by ID"}
          </h2>
          <p className="mt-2 text-xs text-muted-foreground leading-relaxed">
            {isRu
              ? "Введите номер заказа или UUID из чека / письма подтверждения, чтобы перейти к странице статуса."
              : "Enter your order ID or UUID received in your confirmation email to open live tracking."}
          </p>

          <form onSubmit={handleLookupSubmit} className="mt-6 space-y-4">
            <div>
              <input
                type="text"
                value={lookupId}
                onChange={(e) => setLookupId(e.target.value)}
                placeholder={
                  isRu
                    ? "Например: a1b2c3d4-e5f6-7890-..."
                    : "e.g. a1b2c3d4-e5f6-7890-..."
                }
                className="h-11 w-full rounded-xl border border-border bg-background px-4 font-mono text-xs text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary text-center"
              />
              {lookupError && (
                <p className="mt-1.5 text-xs text-destructive">{lookupError}</p>
              )}
            </div>

            <button
              type="submit"
              className="inline-flex h-11 w-full items-center justify-center gap-2 rounded-xl bg-primary px-6 text-sm font-semibold text-primary-foreground hover:bg-primary/90 transition shadow-xs cursor-pointer"
            >
              <span>{isRu ? "Перейти к отслеживанию" : "Open Tracking Page"}</span>
              <ArrowRight className="h-4 w-4" />
            </button>
          </form>
        </div>
      ) : isLoading ? (
        /* Loading skeleton */
        <div className="space-y-4">
          {[1, 2, 3].map((i) => (
            <div
              key={i}
              className="rounded-3xl border border-border bg-card p-6 animate-pulse"
            >
              <div className="flex justify-between items-center mb-4">
                <div className="h-4 w-32 bg-muted rounded-md" />
                <div className="h-6 w-24 bg-muted rounded-full" />
              </div>
              <div className="h-12 bg-muted/50 rounded-xl" />
            </div>
          ))}
        </div>
      ) : filteredOrders.length === 0 ? (
        /* Empty State */
        <div className="rounded-3xl border border-dashed border-border bg-card p-12 text-center">
          <div className="mx-auto mb-4 flex h-14 w-14 items-center justify-center rounded-2xl bg-muted text-muted-foreground">
            <Package className="h-7 w-7" />
          </div>
          <h2 className="text-xl font-bold text-foreground">
            {isRu ? "Заказы не найдены" : "No Orders Found"}
          </h2>
          <p className="mx-auto mt-2 max-w-md text-xs text-muted-foreground">
            {searchQuery
              ? isRu
                ? "По вашему поисковому запросу ничего не найдено. Попробуйте изменить параметры."
                : "No orders match your search criteria. Try a different keyword."
              : isRu
              ? "У вас пока нет оформленных заказов. Выберите товары в каталоге и оформите первый заказ!"
              : "You haven't placed any orders yet. Browse our catalog to place your first order!"}
          </p>
          <div className="mt-6">
            <Link
              href={`/${locale}/products`}
              className="inline-flex h-11 items-center justify-center gap-2 rounded-xl bg-primary px-6 text-sm font-semibold text-primary-foreground hover:bg-primary/90 transition shadow-xs"
            >
              {isRu ? "Перейти в каталог" : "Browse Products"}
            </Link>
          </div>
        </div>
      ) : (
        /* Orders List */
        <div className="space-y-6">
          {filteredOrders.map((order) => {
            const badge = getStatusBadge(order.status);
            const BadgeIcon = badge.icon;
            const currencyCode = order.currencyCode || (isRu ? "RUB" : "USD");
            const total = parseFloat(order.totalAmount || "0");

            return (
              <div
                key={order.id}
                className="overflow-hidden rounded-3xl border border-border bg-card shadow-xs transition hover:border-primary/40"
              >
                {/* Order Card Top Bar */}
                <div className="flex flex-wrap items-center justify-between gap-4 border-b border-border/70 bg-muted/20 px-6 py-4">
                  <div className="flex flex-wrap items-center gap-3">
                    <div>
                      <span className="text-[11px] font-semibold text-muted-foreground block">
                        {isRu ? "Номер заказа" : "Order ID"}
                      </span>
                      <span className="font-mono text-sm font-bold text-foreground">
                        #{order.id.slice(0, 8)}
                      </span>
                    </div>

                    <div className="h-6 w-px bg-border hidden sm:block" />

                    <div>
                      <span className="text-[11px] font-semibold text-muted-foreground block">
                        {isRu ? "Дата оформления" : "Order Date"}
                      </span>
                      <span className="text-xs font-medium text-foreground flex items-center gap-1.5">
                        <Calendar className="h-3 w-3 text-muted-foreground" />
                        {new Date(order.createdAt).toLocaleDateString(locale, {
                          year: "numeric",
                          month: "short",
                          day: "numeric",
                        })}
                      </span>
                    </div>
                  </div>

                  <div className="flex items-center gap-3">
                    <span
                      className={`inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-xs font-bold ${badge.className}`}
                    >
                      <BadgeIcon className="h-3.5 w-3.5" />
                      <span>{badge.label}</span>
                    </span>

                    <Link
                      href={`/${locale}/orders/${order.id}`}
                      className="inline-flex h-9 items-center justify-center gap-1.5 rounded-xl bg-primary px-4 text-xs font-semibold text-primary-foreground hover:bg-primary/90 transition shadow-xs cursor-pointer"
                    >
                      <span>{isRu ? "Трекер заказа" : "Track Order"}</span>
                      <ArrowRight className="h-3.5 w-3.5" />
                    </Link>
                  </div>
                </div>

                {/* Order Items Preview */}
                <div className="p-6">
                  <div className="grid grid-cols-1 gap-6 lg:grid-cols-12 items-center">
                    {/* Items List (8 cols) */}
                    <div className="lg:col-span-8 space-y-3">
                      {order.lineItems.slice(0, 3).map((item) => (
                        <div
                          key={item.id}
                          className="flex items-center justify-between text-xs"
                        >
                          <div className="flex items-center gap-2.5">
                            <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground">
                              <Package className="h-3.5 w-3.5" />
                            </div>
                            <span className="font-semibold text-foreground">
                              {item.title}
                            </span>
                            <span className="text-muted-foreground font-mono">
                              × {item.quantity}
                            </span>
                          </div>
                          <span className="font-medium text-foreground">
                            {parseFloat(item.totalPrice || "0").toFixed(2)}{" "}
                            {item.currencyCode || currencyCode}
                          </span>
                        </div>
                      ))}

                      {order.lineItems.length > 3 && (
                        <p className="text-xs text-muted-foreground italic">
                          {isRu
                            ? `... и ещё ${order.lineItems.length - 3} поз.`
                            : `... and ${order.lineItems.length - 3} more items`}
                        </p>
                      )}
                    </div>

                    {/* Financial Summary & Delivery info (4 cols) */}
                    <div className="lg:col-span-4 border-t lg:border-t-0 lg:border-l border-border pt-4 lg:pt-0 lg:pl-6 space-y-2 text-right">
                      <div>
                        <span className="text-xs text-muted-foreground block">
                          {isRu ? "Итого к оплате" : "Total Amount"}
                        </span>
                        <span className="text-lg font-extrabold text-primary">
                          {total.toFixed(2)} {currencyCode}
                        </span>
                      </div>

                      {order.carrier && (
                        <div className="text-xs text-muted-foreground flex items-center justify-end gap-1.5">
                          <Truck className="h-3.5 w-3.5" />
                          <span>{order.carrier}</span>
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
