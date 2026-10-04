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

import { useMemo, useState } from "react";
import Link from "next/link";
import {
  ArrowLeft,
  Calendar,
  CheckCircle2,
  Clock,
  Copy,
  CreditCard,
  ExternalLink,
  HelpCircle,
  MapPin,
  Package,
  PackageCheck,
  Printer,
  ShieldCheck,
  Truck,
  User,
  XCircle,
} from "lucide-react";
import type { Order, OrderMetadataParsed } from "../api/types";

export interface OrderViewProps {
  order: Order | null;
  orderId: string;
  locale?: string;
}

export function OrderView({
  order,
  orderId,
  locale = "ru",
}: OrderViewProps) {
  const isRu = locale.toLowerCase().startsWith("ru");
  const [copiedTracking, setCopiedTracking] = useState(false);

  // Parse order metadata if available
  const parsedMetadata: OrderMetadataParsed | null = useMemo(() => {
    if (!order?.metadata) return null;
    try {
      return JSON.parse(order.metadata);
    } catch {
      return null;
    }
  }, [order?.metadata]);

  // Order status normalization & stepper calculation
  const status = (order?.status || "PENDING").toUpperCase();

  const isCancelled = status === "CANCELLED";

  const currentStep = useMemo(() => {
    switch (status) {
      case "PENDING":
        return 1;
      case "CONFIRMED":
        return 2;
      case "PROCESSING":
        return 3;
      case "SHIPPED":
        return 4;
      case "DELIVERED":
        return 5;
      default:
        return 1;
    }
  }, [status]);

  const steps = [
    {
      step: 1,
      title: isRu ? "Оформлен" : "Order Placed",
      desc: isRu ? "Заказ получен системой" : "Order received",
      icon: Clock,
      date: order?.createdAt,
    },
    {
      step: 2,
      title: isRu ? "Оплачен" : "Payment Confirmed",
      desc: isRu ? "Оплата подтверждена" : "Payment processed",
      icon: CreditCard,
      date: order?.paidAt || order?.confirmedAt,
    },
    {
      step: 3,
      title: isRu ? "Сборка" : "Processing",
      desc: isRu ? "Комплектуется на складе" : "Packing items",
      icon: Package,
      date: order?.updatedAt,
    },
    {
      step: 4,
      title: isRu ? "В пути" : "In Transit",
      desc: isRu ? "Передан в службу доставки" : "Handed to courier",
      icon: Truck,
      date: order?.shippedAt,
    },
    {
      step: 5,
      title: isRu ? "Доставлен" : "Delivered",
      desc: isRu ? "Вручен получателю" : "Successfully delivered",
      icon: PackageCheck,
      date: order?.deliveredAt,
    },
  ];

  const handleCopyTracking = (code: string) => {
    if (typeof navigator !== "undefined" && navigator.clipboard) {
      void navigator.clipboard.writeText(code);
      setCopiedTracking(true);
      setTimeout(() => setCopiedTracking(false), 2000);
    }
  };

  const handlePrint = () => {
    if (typeof window !== "undefined") {
      window.print();
    }
  };

  const currencyCode = order?.currencyCode || (isRu ? "RUB" : "USD");
  const subtotal = parseFloat(order?.subtotalAmount || "0");
  const discount = parseFloat(order?.adjustmentTotal || "0");
  const shipping = parseFloat(order?.shippingTotal || "0");
  const tax = parseFloat(order?.taxTotal || "0");
  const total = parseFloat(order?.totalAmount || "0");

  const customerName =
    parsedMetadata?.customer?.fullName ||
    (isRu ? "Покупатель" : "Customer");
  const customerEmail = parsedMetadata?.customer?.email || "";
  const customerPhone = parsedMetadata?.customer?.phone || "";

  const shippingCity = parsedMetadata?.shippingAddress?.city || "";
  const shippingStreet = parsedMetadata?.shippingAddress?.streetAddress || "";
  const shippingCountry =
    parsedMetadata?.shippingAddress?.countryCode || (isRu ? "RU" : "US");
  const shippingPostal = parsedMetadata?.shippingAddress?.postalCode || "";

  const paymentMethodLabel = useMemo(() => {
    const raw =
      order?.paymentMethod ||
      parsedMetadata?.paymentMethod ||
      "card";
    switch (raw.toLowerCase()) {
      case "cod":
        return isRu ? "Оплата при получении (COD)" : "Cash on Delivery";
      case "transfer":
        return isRu ? "Банковский перевод" : "Bank Transfer";
      case "card":
      default:
        return isRu ? "Банковская карта онлайн" : "Credit or Debit Card";
    }
  }, [order?.paymentMethod, parsedMetadata?.paymentMethod, isRu]);

  const trackingNumber =
    order?.trackingNumber || (order?.id ? `TRK-${order.id.slice(0, 8).toUpperCase()}` : null);
  const carrier = order?.carrier || (isRu ? "Курьерская служба" : "Standard Logistics");

  // Fallback if order not found in DB
  if (!order) {
    return (
      <div className="mx-auto max-w-4xl px-4 py-16 sm:px-6">
        <div className="rounded-3xl border border-dashed border-border bg-card p-10 text-center">
          <div className="mx-auto mb-4 flex h-14 w-14 items-center justify-center rounded-2xl bg-muted text-muted-foreground">
            <Package className="h-7 w-7" />
          </div>
          <h1 className="text-2xl font-bold text-foreground">
            {isRu ? "Заказ не найден" : "Order Not Found"}
          </h1>
          <p className="mx-auto mt-2 max-w-md text-sm text-muted-foreground">
            {isRu
              ? `Заказ с номером ${orderId} не найден в текущей системе или требует авторизации владельца.`
              : `Order with ID ${orderId} was not found or requires account authorization.`}
          </p>
          <div className="mt-6 flex flex-wrap items-center justify-center gap-3">
            <Link
              href={`/${locale}/products`}
              className="inline-flex h-11 items-center justify-center gap-2 rounded-xl bg-primary px-6 text-sm font-semibold text-primary-foreground hover:bg-primary/90 transition shadow-xs"
            >
              <ArrowLeft className="h-4 w-4" />
              {isRu ? "Перейти в каталог" : "Browse Catalog"}
            </Link>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-7xl px-4 py-10 sm:px-6 lg:px-8">
      {/* Top Header & Navigation */}
      <div className="mb-8 flex flex-wrap items-center justify-between gap-4 border-b border-border pb-5">
        <div>
          <Link
            href={`/${locale}/products`}
            className="inline-flex items-center gap-1.5 text-xs font-semibold text-muted-foreground hover:text-foreground transition"
          >
            <ArrowLeft className="h-3.5 w-3.5" />
            {isRu ? "Вернуться в каталог" : "Back to Catalog"}
          </Link>
          <div className="mt-2 flex flex-wrap items-center gap-3">
            <h1 className="text-2xl font-extrabold tracking-tight text-foreground sm:text-3xl">
              {isRu ? "Заказ" : "Order"} #{order.id.slice(0, 8)}
            </h1>

            <span
              className={`inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-xs font-bold ${
                isCancelled
                  ? "bg-destructive/10 text-destructive"
                  : status === "DELIVERED"
                  ? "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400"
                  : "bg-primary/10 text-primary"
              }`}
            >
              {isCancelled ? (
                <>
                  <XCircle className="h-3.5 w-3.5" />
                  {isRu ? "Отменен" : "Cancelled"}
                </>
              ) : status === "DELIVERED" ? (
                <>
                  <CheckCircle2 className="h-3.5 w-3.5" />
                  {isRu ? "Доставлен" : "Delivered"}
                </>
              ) : (
                <>
                  <Clock className="h-3.5 w-3.5" />
                  {isRu ? "В обработке" : "In Progress"}
                </>
              )}
            </span>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={handlePrint}
            className="inline-flex h-9 items-center justify-center gap-2 rounded-xl border border-border bg-card px-3 text-xs font-semibold text-foreground hover:bg-muted transition cursor-pointer"
          >
            <Printer className="h-3.5 w-3.5" />
            <span>{isRu ? "Печать квитанции" : "Print Receipt"}</span>
          </button>
        </div>
      </div>

      {/* Main Grid: Status Tracker & Content */}
      <div className="space-y-8">
        {/* Status Tracker Bar (Timeline) */}
        {!isCancelled && (
          <div className="rounded-3xl border border-border bg-card p-6 sm:p-8 shadow-xs">
            <div className="mb-6 flex items-center justify-between">
              <h2 className="text-base font-bold text-foreground">
                {isRu ? "Статус выполнения заказа" : "Order Progress Tracking"}
              </h2>
              {trackingNumber && (
                <div className="flex items-center gap-2 text-xs">
                  <span className="text-muted-foreground">
                    {isRu ? "Трек-номер:" : "Tracking:"}
                  </span>
                  <span className="font-mono font-semibold text-foreground">
                    {trackingNumber}
                  </span>
                  <button
                    type="button"
                    onClick={() => handleCopyTracking(trackingNumber)}
                    className="p-1 text-muted-foreground hover:text-foreground transition cursor-pointer"
                    title={isRu ? "Скопировать" : "Copy"}
                  >
                    <Copy className="h-3.5 w-3.5" />
                  </button>
                  {copiedTracking && (
                    <span className="text-emerald-500 font-semibold text-[10px]">
                      {isRu ? "Скопировано!" : "Copied!"}
                    </span>
                  )}
                </div>
              )}
            </div>

            {/* Stepper Graphic */}
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-5 relative">
              {steps.map((st, idx) => {
                const Icon = st.icon;
                const isPassed = currentStep >= st.step;
                const isCurrent = currentStep === st.step;

                return (
                  <div
                    key={st.step}
                    className={`relative flex flex-col p-4 rounded-2xl border transition ${
                      isCurrent
                        ? "border-primary bg-primary/5 ring-1 ring-primary shadow-xs"
                        : isPassed
                        ? "border-border/80 bg-background/60"
                        : "border-border/40 bg-muted/20 opacity-60"
                    }`}
                  >
                    <div className="flex items-center justify-between mb-2">
                      <div
                        className={`flex h-8 w-8 items-center justify-center rounded-xl text-xs font-bold ${
                          isPassed
                            ? "bg-primary text-primary-foreground"
                            : "bg-muted text-muted-foreground"
                        }`}
                      >
                        <Icon className="h-4 w-4" />
                      </div>
                      <span className="text-[11px] font-bold text-muted-foreground">
                        0{st.step}
                      </span>
                    </div>

                    <h3 className="text-sm font-bold text-foreground">
                      {st.title}
                    </h3>
                    <p className="mt-1 text-xs text-muted-foreground">
                      {st.desc}
                    </p>

                    {st.date && (
                      <span className="mt-3 text-[10px] text-muted-foreground/90 font-mono">
                        {new Date(st.date).toLocaleDateString(locale, {
                          month: "short",
                          day: "numeric",
                          hour: "2-digit",
                          minute: "2-digit",
                        })}
                      </span>
                    )}
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {/* 2-Columns Details Layout */}
        <div className="grid grid-cols-1 gap-8 lg:grid-cols-12">
          {/* Left Column: Items & Customer Details (8 cols) */}
          <div className="space-y-6 lg:col-span-7 xl:col-span-8">
            {/* Items List */}
            <div className="rounded-3xl border border-border bg-card p-6 shadow-xs">
              <h3 className="text-base font-bold text-foreground pb-4 border-b border-border/80 flex items-center justify-between">
                <span>{isRu ? "Состав заказа" : "Ordered Items"}</span>
                <span className="text-xs font-normal text-muted-foreground">
                  {order.lineItems.length}{" "}
                  {isRu ? "поз." : "items"}
                </span>
              </h3>

              <div className="divide-y divide-border/60">
                {order.lineItems.map((item) => (
                  <div
                    key={item.id}
                    className="py-4 flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4"
                  >
                    <div className="flex items-center gap-3">
                      <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl bg-muted text-muted-foreground">
                        <Package className="h-6 w-6" />
                      </div>
                      <div>
                        <h4 className="text-sm font-semibold text-foreground">
                          {item.title}
                        </h4>
                        <div className="mt-0.5 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                          {item.sku && (
                            <span>
                              {isRu ? "Арт:" : "SKU:"} {item.sku}
                            </span>
                          )}
                          <span>•</span>
                          <span>
                            {item.quantity} x{" "}
                            {parseFloat(item.unitPrice || "0").toFixed(2)}{" "}
                            {item.currencyCode || currencyCode}
                          </span>
                        </div>
                      </div>
                    </div>

                    <div className="text-right sm:text-right w-full sm:w-auto">
                      <span className="text-sm font-bold text-foreground">
                        {parseFloat(item.totalPrice || "0").toFixed(2)}{" "}
                        {item.currencyCode || currencyCode}
                      </span>
                    </div>
                  </div>
                ))}
              </div>
            </div>

            {/* Delivery & Shipping Info Card */}
            <div className="grid grid-cols-1 gap-6 sm:grid-cols-2">
              <div className="rounded-3xl border border-border bg-card p-6 shadow-xs">
                <div className="flex items-center gap-2 text-primary pb-3 border-b border-border/60">
                  <MapPin className="h-4 w-4" />
                  <h3 className="text-xs font-bold uppercase tracking-wider text-foreground">
                    {isRu ? "Адрес доставки" : "Delivery Address"}
                  </h3>
                </div>

                <div className="mt-4 space-y-1 text-sm text-foreground">
                  <span className="font-semibold block">{customerName}</span>
                  {customerPhone && (
                    <span className="text-xs text-muted-foreground block">
                      {customerPhone}
                    </span>
                  )}
                  {customerEmail && (
                    <span className="text-xs text-muted-foreground block">
                      {customerEmail}
                    </span>
                  )}
                  <p className="mt-2 text-xs text-muted-foreground leading-relaxed">
                    {shippingCity}
                    {shippingStreet ? `, ${shippingStreet}` : ""}
                    {shippingPostal ? `, ${shippingPostal}` : ""}
                    {shippingCountry ? ` (${shippingCountry})` : ""}
                  </p>
                </div>
              </div>

              {/* Shipping Method & Carrier */}
              <div className="rounded-3xl border border-border bg-card p-6 shadow-xs">
                <div className="flex items-center gap-2 text-primary pb-3 border-b border-border/60">
                  <Truck className="h-4 w-4" />
                  <h3 className="text-xs font-bold uppercase tracking-wider text-foreground">
                    {isRu ? "Служба доставки" : "Fulfillment & Courier"}
                  </h3>
                </div>

                <div className="mt-4 space-y-2 text-sm">
                  <div>
                    <span className="text-xs text-muted-foreground block">
                      {isRu ? "Перевозчик" : "Carrier"}
                    </span>
                    <span className="font-semibold text-foreground">
                      {carrier}
                    </span>
                  </div>

                  {trackingNumber && (
                    <div>
                      <span className="text-xs text-muted-foreground block">
                        {isRu ? "Трек-номер отправления" : "Tracking Number"}
                      </span>
                      <span className="font-mono text-xs font-bold text-primary">
                        {trackingNumber}
                      </span>
                    </div>
                  )}

                  <div>
                    <span className="text-xs text-muted-foreground block">
                      {isRu ? "Ориентировочный срок" : "Estimated Arrival"}
                    </span>
                    <span className="text-xs font-medium text-foreground">
                      {isRu ? "1–3 рабочих дня" : "1–3 business days"}
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </div>

          {/* Right Column: Payment & Price Summary (4 cols) */}
          <div className="space-y-6 lg:col-span-5 xl:col-span-4">
            {/* Price Breakdown Card */}
            <div className="rounded-3xl border border-border bg-card p-6 shadow-xs">
              <h3 className="text-base font-bold text-foreground pb-4 border-b border-border/80">
                {isRu ? "Финансовая сводка" : "Payment Summary"}
              </h3>

              <div className="mt-4 space-y-2.5 text-xs">
                <div className="flex justify-between text-muted-foreground">
                  <span>{isRu ? "Товары" : "Subtotal"}</span>
                  <span className="font-medium text-foreground">
                    {subtotal.toFixed(2)} {currencyCode}
                  </span>
                </div>

                {discount > 0 && (
                  <div className="flex justify-between text-destructive">
                    <span>{isRu ? "Скидка" : "Discount"}</span>
                    <span className="font-semibold">
                      -{discount.toFixed(2)} {currencyCode}
                    </span>
                  </div>
                )}

                <div className="flex justify-between text-muted-foreground">
                  <span>{isRu ? "Доставка" : "Shipping"}</span>
                  <span className="font-medium text-foreground">
                    {shipping === 0
                      ? isRu
                        ? "Бесплатно"
                        : "Free"
                      : `${shipping.toFixed(2)} ${currencyCode}`}
                  </span>
                </div>

                {tax > 0 && (
                  <div className="flex justify-between text-muted-foreground">
                    <span>{isRu ? "НДС / Налог" : "Tax"}</span>
                    <span className="font-medium text-foreground">
                      {tax.toFixed(2)} {currencyCode}
                    </span>
                  </div>
                )}

                <div className="flex justify-between border-t border-border pt-3 text-sm font-extrabold text-foreground">
                  <span>{isRu ? "Итого оплачено" : "Total Amount"}</span>
                  <span className="text-lg text-primary">
                    {total.toFixed(2)} {currencyCode}
                  </span>
                </div>
              </div>

              {/* Payment Method Badge */}
              <div className="mt-6 rounded-2xl border border-border/80 bg-background/50 p-4">
                <div className="flex items-center gap-2 text-xs font-semibold text-foreground">
                  <CreditCard className="h-4 w-4 text-primary" />
                  <span>{paymentMethodLabel}</span>
                </div>
                <div className="mt-1 flex items-center justify-between text-[11px] text-muted-foreground">
                  <span>{isRu ? "Статус оплаты:" : "Payment Status:"}</span>
                  <span className="font-bold text-emerald-600 dark:text-emerald-400">
                    {order.paidAt
                      ? isRu
                        ? "Оплачено"
                        : "Paid"
                      : isRu
                      ? "Ожидает оплаты"
                      : "Pending"}
                  </span>
                </div>
              </div>

              {/* Help & Support CTA */}
              <div className="mt-6 border-t border-border/60 pt-4 text-xs text-muted-foreground">
                <div className="flex items-center gap-2">
                  <HelpCircle className="h-4 w-4 text-primary shrink-0" />
                  <span>
                    {isRu
                      ? "Возникли вопросы по заказу? Наша поддержка работает 24/7."
                      : "Have questions about your order? Support is available 24/7."}
                  </span>
                </div>
              </div>
            </div>

            {/* Guarantees Box */}
            <div className="rounded-3xl border border-border bg-card p-6 shadow-xs space-y-3 text-xs text-muted-foreground">
              <div className="flex items-center gap-2.5">
                <ShieldCheck className="h-4 w-4 text-emerald-500 shrink-0" />
                <span className="text-foreground font-semibold">
                  {isRu
                    ? "Гарантия подлинности и качества"
                    : "Authenticity and Quality Guarantee"}
                </span>
              </div>
              <div className="flex items-center gap-2.5">
                <Package className="h-4 w-4 text-primary shrink-0" />
                <span>
                  {isRu
                    ? "14 дней на возврат товара"
                    : "14-day hassle-free return policy"}
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
