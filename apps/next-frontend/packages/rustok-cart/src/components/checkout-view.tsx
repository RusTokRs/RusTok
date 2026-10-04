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
  AlertCircle,
  ArrowLeft,
  CheckCircle2,
  CreditCard,
  HelpCircle,
  Loader2,
  Lock,
  Package,
  ShieldCheck,
  Truck,
  Wallet,
} from "lucide-react";
import { storefrontGraphql } from "@/shared/lib/graphql";
import { completeStorefrontCheckout } from "../api/cart";
import type {
  CartShippingOptionSummary,
  CompleteCheckoutResult,
  CustomerShippingAddress,
} from "../api/types";
import { useCart } from "../context/cart-context";

export interface CheckoutViewProps {
  locale?: string;
  tenantSlug?: string | null;
}

export function CheckoutView({
  locale = "ru",
  tenantSlug = null,
}: CheckoutViewProps) {
  const isRu = locale.toLowerCase().startsWith("ru");
  const { cart, isLoading, clearCart, selectShippingOption } = useCart();

  // Form State
  const [formData, setFormData] = useState<CustomerShippingAddress>({
    fullName: "",
    email: cart?.email || "",
    phone: "",
    countryCode: cart?.countryCode || (isRu ? "RU" : "US"),
    city: "",
    streetAddress: "",
    postalCode: "",
    paymentMethod: "card",
    notes: "",
  });

  const [selectedShippingOptionId, setSelectedShippingOptionId] =
    useState<string>(cart?.selectedShippingOptionId || "");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [completedOrder, setCompletedOrder] =
    useState<CompleteCheckoutResult | null>(null);

  // Available shipping options from cart delivery groups
  const availableShippingOptions: CartShippingOptionSummary[] = useMemo(() => {
    if (!cart?.deliveryGroups || cart.deliveryGroups.length === 0) {
      // Default fallback options if none configured in backend
      return [
        {
          id: "standard-courier",
          name: isRu ? "Курьерская доставка" : "Standard Courier Delivery",
          currencyCode: cart?.currencyCode || "RUB",
          amount: "350.00",
          providerId: "standard-carrier",
          active: true,
        },
        {
          id: "express-courier",
          name: isRu ? "Экспресс доставка (1-2 дня)" : "Express Delivery (1-2 days)",
          currencyCode: cart?.currencyCode || "RUB",
          amount: "650.00",
          providerId: "express-carrier",
          active: true,
        },
        {
          id: "pickup-point",
          name: isRu ? "Пункт выдачи заказов (ПВЗ)" : "Pickup Point",
          currencyCode: cart?.currencyCode || "RUB",
          amount: "190.00",
          providerId: "pickup-carrier",
          active: true,
        },
      ];
    }

    const options: CartShippingOptionSummary[] = [];
    for (const group of cart.deliveryGroups) {
      for (const option of group.availableShippingOptions) {
        if (!options.some((o) => o.id === option.id)) {
          options.push(option);
        }
      }
    }
    return options.length > 0 ? options : [
      {
        id: "standard",
        name: isRu ? "Стандартная доставка" : "Standard Delivery",
        currencyCode: cart.currencyCode,
        amount: cart.shippingTotal || "0.00",
        providerId: "default",
        active: true,
      },
    ];
  }, [cart?.deliveryGroups, cart?.currencyCode, cart?.shippingTotal, isRu]);

  // Set default shipping option if not selected
  const activeShippingId =
    selectedShippingOptionId ||
    availableShippingOptions[0]?.id ||
    "standard";

  const selectedShippingOption =
    availableShippingOptions.find((o) => o.id === activeShippingId) ||
    availableShippingOptions[0];

  const currencyCode = cart?.currencyCode || (isRu ? "RUB" : "USD");

  const subtotal = parseFloat(cart?.subtotalAmount || "0");
  const discount = parseFloat(cart?.adjustmentTotal || "0");
  const shippingAmount = selectedShippingOption
    ? parseFloat(selectedShippingOption.amount || "0")
    : parseFloat(cart?.shippingTotal || "0");
  const grandTotal = Math.max(0, subtotal - discount + shippingAmount);

  const handleInputChange = (
    e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement>,
  ) => {
    const { name, value } = e.target;
    setFormData((prev) => ({ ...prev, [name]: value }));
  };

  const handleShippingOptionChange = async (optionId: string) => {
    setSelectedShippingOptionId(optionId);
    if (cart?.id) {
      await selectShippingOption(optionId);
    }
  };

  const handleSubmitOrder = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!cart?.id || !cart.lineItems || cart.lineItems.length === 0) {
      setErrorMessage(
        isRu
          ? "Корзина пуста. Невозможно оформить заказ."
          : "Cart is empty. Cannot complete checkout.",
      );
      return;
    }

    // Validation
    if (!formData.fullName.trim()) {
      setErrorMessage(
        isRu ? "Пожалуйста, введите ваше имя" : "Please enter your full name",
      );
      return;
    }
    if (!formData.email.trim() || !formData.email.includes("@")) {
      setErrorMessage(
        isRu
          ? "Пожалуйста, укажите корректный email"
          : "Please enter a valid email address",
      );
      return;
    }
    if (!formData.phone.trim()) {
      setErrorMessage(
        isRu
          ? "Пожалуйста, введите контактный телефон"
          : "Please enter your phone number",
      );
      return;
    }
    if (!formData.city.trim() || !formData.streetAddress.trim()) {
      setErrorMessage(
        isRu
          ? "Пожалуйста, заполните адрес доставки"
          : "Please fill in your delivery address",
      );
      return;
    }

    setIsSubmitting(true);
    setErrorMessage(null);

    try {
      // Build checkout metadata containing shipping & customer details
      const checkoutMetadata = JSON.stringify({
        customer: {
          fullName: formData.fullName.trim(),
          email: formData.email.trim(),
          phone: formData.phone.trim(),
        },
        shippingAddress: {
          countryCode: formData.countryCode,
          city: formData.city.trim(),
          streetAddress: formData.streetAddress.trim(),
          postalCode: formData.postalCode.trim(),
        },
        shippingMethod: {
          id: activeShippingId,
          name: selectedShippingOption?.name,
          amount: selectedShippingOption?.amount,
        },
        paymentMethod: formData.paymentMethod,
        notes: formData.notes?.trim() || null,
      });

      // Prepare shipping selections for backend
      const shippingSelections = cart.deliveryGroups?.map((group) => ({
        shippingProfileSlug: group.shippingProfileSlug,
        sellerId: group.sellerId ?? null,
        selectedShippingOptionId: activeShippingId,
      })) ?? [];

      const result = await completeStorefrontCheckout(
        storefrontGraphql,
        {
          cartId: cart.id,
          shippingOptionId: activeShippingId.includes("-") ? activeShippingId : null,
          shippingSelections:
            shippingSelections.length > 0 ? shippingSelections : undefined,
          countryCode: formData.countryCode,
          locale,
          createFulfillment: true,
          metadata: checkoutMetadata,
        },
        tenantSlug,
      );

      if (result) {
        setCompletedOrder(result);
        clearCart();
      } else {
        setErrorMessage(
          isRu
            ? "Не удалось завершить оформление. Пожалуйста, попробуйте снова."
            : "Failed to complete checkout. Please try again.",
        );
      }
    } catch (err: unknown) {
      console.error("Checkout error:", err);
      const message =
        err instanceof Error ? err.message : String(err);
      setErrorMessage(
        message ||
          (isRu
            ? "Произошла ошибка при оформлении заказа. Проверьте данные и повторите."
            : "An error occurred while placing the order. Please verify details and retry."),
      );
    } finally {
      setIsSubmitting(false);
    }
  };

  // 1. Success confirmation screen
  if (completedOrder) {
    const order = completedOrder.order;
    return (
      <div className="mx-auto max-w-3xl px-4 py-16 sm:px-6">
        <div className="overflow-hidden rounded-3xl border border-border bg-card p-8 shadow-sm text-center">
          <div className="mx-auto mb-6 flex h-16 w-16 items-center justify-center rounded-2xl bg-emerald-500/10 text-emerald-500">
            <CheckCircle2 className="h-10 w-10" />
          </div>

          <span className="inline-flex items-center rounded-full bg-emerald-500/10 px-3 py-1 text-xs font-semibold text-emerald-600 dark:text-emerald-400">
            {isRu ? "Заказ успешно оформлен" : "Order Placed Successfully"}
          </span>

          <h1 className="mt-4 text-3xl font-extrabold tracking-tight text-foreground sm:text-4xl">
            {isRu ? "Спасибо за покупку!" : "Thank You for Your Order!"}
          </h1>

          <p className="mt-2 text-sm text-muted-foreground">
            {isRu
              ? `Мы отправили подтверждение на адрес ${formData.email}. Наш менеджер свяжется с вами для уточнения деталей.`
              : `A confirmation email has been sent to ${formData.email}. We will update you with delivery progress.`}
          </p>

          <div className="mt-8 rounded-2xl border border-border/80 bg-background/50 p-6 text-left">
            <h3 className="text-xs font-bold uppercase tracking-wider text-muted-foreground">
              {isRu ? "Информация о заказе" : "Order Details"}
            </h3>

            <div className="mt-4 grid grid-cols-1 gap-4 sm:grid-cols-2 text-sm">
              <div>
                <span className="text-xs text-muted-foreground block">
                  {isRu ? "Номер заказа" : "Order ID"}
                </span>
                <span className="font-mono font-semibold text-foreground">
                  {order.id}
                </span>
              </div>

              <div>
                <span className="text-xs text-muted-foreground block">
                  {isRu ? "Статус заказа" : "Status"}
                </span>
                <span className="inline-flex items-center rounded-md bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary">
                  {order.status}
                </span>
              </div>

              <div>
                <span className="text-xs text-muted-foreground block">
                  {isRu ? "Получатель" : "Customer"}
                </span>
                <span className="font-medium text-foreground">
                  {formData.fullName} ({formData.phone})
                </span>
              </div>

              <div>
                <span className="text-xs text-muted-foreground block">
                  {isRu ? "Адрес доставки" : "Shipping Destination"}
                </span>
                <span className="font-medium text-foreground">
                  {formData.city}, {formData.streetAddress}
                  {formData.postalCode ? `, ${formData.postalCode}` : ""}
                </span>
              </div>

              <div className="sm:col-span-2 border-t border-border pt-3 flex justify-between items-center font-bold">
                <span>{isRu ? "Итого к оплате" : "Total Amount"}</span>
                <span className="text-base text-primary">
                  {parseFloat(order.totalAmount || String(grandTotal)).toFixed(2)}{" "}
                  {order.currencyCode || currencyCode}
                </span>
              </div>
            </div>
          </div>

          <div className="mt-8 flex flex-col sm:flex-row items-center justify-center gap-3">
            <Link
              href={`/${locale}/orders/${order.id}`}
              className="w-full sm:w-auto inline-flex h-11 items-center justify-center rounded-xl bg-primary px-6 text-sm font-semibold text-primary-foreground hover:bg-primary/90 transition shadow-xs"
            >
              {isRu ? "Отследить заказ" : "Track Order Status"}
            </Link>
            <Link
              href={`/${locale}/products`}
              className="w-full sm:w-auto inline-flex h-11 items-center justify-center rounded-xl border border-border px-6 text-sm font-semibold text-foreground hover:bg-muted transition"
            >
              {isRu ? "Продолжить покупки" : "Continue Shopping"}
            </Link>
            <Link
              href={`/${locale}`}
              className="w-full sm:w-auto inline-flex h-11 items-center justify-center rounded-xl border border-border px-6 text-sm font-semibold text-foreground hover:bg-muted transition"
            >
              {isRu ? "На главную" : "Back to Home"}
            </Link>
          </div>
        </div>
      </div>
    );
  }

  // 2. Loading state
  if (isLoading && !cart) {
    return (
      <div className="mx-auto max-w-4xl px-4 py-24 text-center">
        <Loader2 className="mx-auto h-8 w-8 animate-spin text-primary" />
        <p className="mt-4 text-sm text-muted-foreground">
          {isRu ? "Загрузка информации о заказе..." : "Loading checkout details..."}
        </p>
      </div>
    );
  }

  // 3. Empty cart view
  if (!cart || !cart.lineItems || cart.lineItems.length === 0) {
    return (
      <div className="mx-auto max-w-3xl px-4 py-16 text-center">
        <div className="rounded-3xl border border-dashed border-border bg-card p-12">
          <div className="mx-auto mb-4 flex h-14 w-14 items-center justify-center rounded-2xl bg-muted text-muted-foreground">
            <Package className="h-7 w-7" />
          </div>
          <h2 className="text-2xl font-bold text-foreground">
            {isRu ? "Ваша корзина пуста" : "Your cart is empty"}
          </h2>
          <p className="mx-auto mt-2 max-w-md text-sm text-muted-foreground">
            {isRu
              ? "Добавьте товары или готовые комплекты в корзину перед тем, как перейти к оформлению заказа."
              : "Add products or bundles to your cart before proceeding to checkout."}
          </p>
          <div className="mt-6">
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

  // 4. Main Checkout Form
  return (
    <div className="mx-auto max-w-7xl px-4 py-10 sm:px-6 lg:px-8">
      {/* Top Navigation & Title */}
      <div className="mb-8 flex flex-wrap items-center justify-between gap-4 border-b border-border pb-5">
        <div>
          <Link
            href={`/${locale}/products`}
            className="inline-flex items-center gap-1.5 text-xs font-semibold text-muted-foreground hover:text-foreground transition"
          >
            <ArrowLeft className="h-3.5 w-3.5" />
            {isRu ? "Вернуться к покупкам" : "Continue Shopping"}
          </Link>
          <h1 className="mt-2 text-2xl font-extrabold tracking-tight text-foreground sm:text-3xl">
            {isRu ? "Оформление заказа" : "Checkout"}
          </h1>
        </div>

        <div className="flex items-center gap-2 rounded-xl border border-border/80 bg-secondary/50 px-3 py-1.5 text-xs text-muted-foreground">
          <Lock className="h-3.5 w-3.5 text-emerald-500" />
          <span>{isRu ? "Безопасное соединение SSL" : "Secure 256-bit SSL"}</span>
        </div>
      </div>

      {errorMessage && (
        <div className="mb-6 flex items-start gap-3 rounded-2xl border border-destructive/30 bg-destructive/10 p-4 text-sm text-destructive">
          <AlertCircle className="h-5 w-5 shrink-0 mt-0.5" />
          <div>
            <span className="font-semibold block">
              {isRu ? "Внимание" : "Checkout error"}
            </span>
            <span>{errorMessage}</span>
          </div>
        </div>
      )}

      <form onSubmit={handleSubmitOrder} className="grid grid-cols-1 gap-8 lg:grid-cols-12">
        {/* Left Column: Checkout Inputs (8 cols) */}
        <div className="space-y-8 lg:col-span-7 xl:col-span-8">
          {/* Section 1: Contact Information */}
          <div className="rounded-3xl border border-border bg-card p-6 sm:p-8 shadow-xs">
            <div className="flex items-center gap-3 border-b border-border/60 pb-4">
              <span className="flex h-7 w-7 items-center justify-center rounded-full bg-primary/10 text-xs font-bold text-primary">
                1
              </span>
              <h2 className="text-lg font-bold text-foreground">
                {isRu ? "Контактные данные" : "Contact Information"}
              </h2>
            </div>

            <div className="mt-6 grid grid-cols-1 gap-4 sm:grid-cols-2">
              <div className="sm:col-span-2">
                <label className="mb-1.5 block text-xs font-semibold text-foreground">
                  {isRu ? "Имя и фамилия" : "Full Name"}{" "}
                  <span className="text-destructive">*</span>
                </label>
                <input
                  type="text"
                  name="fullName"
                  required
                  placeholder={isRu ? "Иван Иванов" : "John Doe"}
                  value={formData.fullName}
                  onChange={handleInputChange}
                  className="w-full rounded-xl border border-input bg-background px-3.5 py-2.5 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
                />
              </div>

              <div>
                <label className="mb-1.5 block text-xs font-semibold text-foreground">
                  {isRu ? "Электронная почта" : "Email Address"}{" "}
                  <span className="text-destructive">*</span>
                </label>
                <input
                  type="email"
                  name="email"
                  required
                  placeholder="name@example.com"
                  value={formData.email}
                  onChange={handleInputChange}
                  className="w-full rounded-xl border border-input bg-background px-3.5 py-2.5 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
                />
              </div>

              <div>
                <label className="mb-1.5 block text-xs font-semibold text-foreground">
                  {isRu ? "Номер телефона" : "Phone Number"}{" "}
                  <span className="text-destructive">*</span>
                </label>
                <input
                  type="tel"
                  name="phone"
                  required
                  placeholder="+7 (999) 000-00-00"
                  value={formData.phone}
                  onChange={handleInputChange}
                  className="w-full rounded-xl border border-input bg-background px-3.5 py-2.5 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
                />
              </div>
            </div>
          </div>

          {/* Section 2: Delivery Address */}
          <div className="rounded-3xl border border-border bg-card p-6 sm:p-8 shadow-xs">
            <div className="flex items-center gap-3 border-b border-border/60 pb-4">
              <span className="flex h-7 w-7 items-center justify-center rounded-full bg-primary/10 text-xs font-bold text-primary">
                2
              </span>
              <h2 className="text-lg font-bold text-foreground">
                {isRu ? "Адрес доставки" : "Delivery Address"}
              </h2>
            </div>

            <div className="mt-6 grid grid-cols-1 gap-4 sm:grid-cols-2">
              <div>
                <label className="mb-1.5 block text-xs font-semibold text-foreground">
                  {isRu ? "Страна" : "Country"}
                </label>
                <select
                  name="countryCode"
                  value={formData.countryCode}
                  onChange={handleInputChange}
                  className="w-full rounded-xl border border-input bg-background px-3.5 py-2.5 text-sm text-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
                >
                  <option value="RU">
                    {isRu ? "Россия (RU)" : "Russian Federation (RU)"}
                  </option>
                  <option value="BY">
                    {isRu ? "Беларусь (BY)" : "Belarus (BY)"}
                  </option>
                  <option value="KZ">
                    {isRu ? "Казахстан (KZ)" : "Kazakhstan (KZ)"}
                  </option>
                  <option value="US">
                    {isRu ? "США (US)" : "United States (US)"}
                  </option>
                  <option value="DE">
                    {isRu ? "Германия (DE)" : "Germany (DE)"}
                  </option>
                </select>
              </div>

              <div>
                <label className="mb-1.5 block text-xs font-semibold text-foreground">
                  {isRu ? "Город / Населенный пункт" : "City"}{" "}
                  <span className="text-destructive">*</span>
                </label>
                <input
                  type="text"
                  name="city"
                  required
                  placeholder={isRu ? "Москва" : "New York"}
                  value={formData.city}
                  onChange={handleInputChange}
                  className="w-full rounded-xl border border-input bg-background px-3.5 py-2.5 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
                />
              </div>

              <div className="sm:col-span-2">
                <label className="mb-1.5 block text-xs font-semibold text-foreground">
                  {isRu ? "Улица, дом, квартира" : "Street Address & Apartment"}{" "}
                  <span className="text-destructive">*</span>
                </label>
                <input
                  type="text"
                  name="streetAddress"
                  required
                  placeholder={
                    isRu
                      ? "ул. Тверская, д. 10, кв. 25"
                      : "123 Main St, Apt 4B"
                  }
                  value={formData.streetAddress}
                  onChange={handleInputChange}
                  className="w-full rounded-xl border border-input bg-background px-3.5 py-2.5 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
                />
              </div>

              <div>
                <label className="mb-1.5 block text-xs font-semibold text-foreground">
                  {isRu ? "Почтовый индекс" : "Postal / ZIP Code"}
                </label>
                <input
                  type="text"
                  name="postalCode"
                  placeholder="101000"
                  value={formData.postalCode}
                  onChange={handleInputChange}
                  className="w-full rounded-xl border border-input bg-background px-3.5 py-2.5 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
                />
              </div>

              <div>
                <label className="mb-1.5 block text-xs font-semibold text-foreground">
                  {isRu ? "Комментарий к заказу" : "Order Notes"}
                </label>
                <input
                  type="text"
                  name="notes"
                  placeholder={
                    isRu
                      ? "Код домофона, удобное время доставки"
                      : "Door code, delivery instructions"
                  }
                  value={formData.notes}
                  onChange={handleInputChange}
                  className="w-full rounded-xl border border-input bg-background px-3.5 py-2.5 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
                />
              </div>
            </div>
          </div>

          {/* Section 3: Delivery Options */}
          <div className="rounded-3xl border border-border bg-card p-6 sm:p-8 shadow-xs">
            <div className="flex items-center gap-3 border-b border-border/60 pb-4">
              <span className="flex h-7 w-7 items-center justify-center rounded-full bg-primary/10 text-xs font-bold text-primary">
                3
              </span>
              <h2 className="text-lg font-bold text-foreground">
                {isRu ? "Способ доставки" : "Delivery Method"}
              </h2>
            </div>

            <div className="mt-6 space-y-3">
              {availableShippingOptions.map((option) => {
                const isSelected = option.id === activeShippingId;
                const optionPrice = parseFloat(option.amount || "0");
                return (
                  <label
                    key={option.id}
                    onClick={() => handleShippingOptionChange(option.id)}
                    className={`flex items-center justify-between p-4 rounded-2xl border transition cursor-pointer ${
                      isSelected
                        ? "border-primary bg-primary/5 ring-1 ring-primary"
                        : "border-border bg-background/60 hover:bg-muted/40"
                    }`}
                  >
                    <div className="flex items-center gap-3">
                      <input
                        type="radio"
                        name="shippingOption"
                        checked={isSelected}
                        onChange={() => handleShippingOptionChange(option.id)}
                        className="h-4 w-4 text-primary focus:ring-primary"
                      />
                      <div>
                        <span className="text-sm font-semibold text-foreground block">
                          {option.name}
                        </span>
                        <span className="text-xs text-muted-foreground flex items-center gap-1 mt-0.5">
                          <Truck className="h-3 w-3" />
                          {option.providerId}
                        </span>
                      </div>
                    </div>

                    <div className="text-right">
                      <span className="text-sm font-bold text-foreground">
                        {optionPrice === 0
                          ? isRu
                            ? "Бесплатно"
                            : "Free"
                          : `${optionPrice.toFixed(2)} ${option.currencyCode || currencyCode}`}
                      </span>
                    </div>
                  </label>
                );
              })}
            </div>
          </div>

          {/* Section 4: Payment Method */}
          <div className="rounded-3xl border border-border bg-card p-6 sm:p-8 shadow-xs">
            <div className="flex items-center gap-3 border-b border-border/60 pb-4">
              <span className="flex h-7 w-7 items-center justify-center rounded-full bg-primary/10 text-xs font-bold text-primary">
                4
              </span>
              <h2 className="text-lg font-bold text-foreground">
                {isRu ? "Способ оплаты" : "Payment Method"}
              </h2>
            </div>

            <div className="mt-6 space-y-3">
              {[
                {
                  id: "card",
                  title: isRu ? "Банковская карта онлайн" : "Credit or Debit Card",
                  desc: isRu
                    ? "Мир, Visa, Mastercard через защищенный платежный шлюз"
                    : "Secure card processing via gateway",
                  icon: CreditCard,
                },
                {
                  id: "cod",
                  title: isRu ? "Оплата при получении" : "Cash on Delivery (COD)",
                  desc: isRu
                    ? "Оплата наличными или картой курьеру при вручении"
                    : "Pay with cash or card upon receiving order",
                  icon: Wallet,
                },
                {
                  id: "transfer",
                  title: isRu ? "Банковский перевод / Счет" : "Bank Transfer",
                  desc: isRu
                    ? "Оплата по банковским реквизитам или счету"
                    : "Invoice and bank wire transfer",
                  icon: HelpCircle,
                },
              ].map((method) => {
                const isSelected = formData.paymentMethod === method.id;
                const Icon = method.icon;
                return (
                  <label
                    key={method.id}
                    onClick={() =>
                      setFormData((prev) => ({
                        ...prev,
                        paymentMethod: method.id as "card" | "cod" | "transfer",
                      }))
                    }
                    className={`flex items-start gap-3 p-4 rounded-2xl border transition cursor-pointer ${
                      isSelected
                        ? "border-primary bg-primary/5 ring-1 ring-primary"
                        : "border-border bg-background/60 hover:bg-muted/40"
                    }`}
                  >
                    <input
                      type="radio"
                      name="paymentMethod"
                      checked={isSelected}
                      onChange={() =>
                        setFormData((prev) => ({
                          ...prev,
                          paymentMethod: method.id as "card" | "cod" | "transfer",
                        }))
                      }
                      className="h-4 w-4 mt-1 text-primary focus:ring-primary"
                    />
                    <div className="flex-1">
                      <div className="flex items-center gap-2">
                        <Icon className="h-4 w-4 text-primary" />
                        <span className="text-sm font-semibold text-foreground">
                          {method.title}
                        </span>
                      </div>
                      <p className="mt-0.5 text-xs text-muted-foreground">
                        {method.desc}
                      </p>
                    </div>
                  </label>
                );
              })}
            </div>
          </div>
        </div>

        {/* Right Column: Order Summary & Placement (4 cols) */}
        <div className="lg:col-span-5 xl:col-span-4">
          <div className="sticky top-6 rounded-3xl border border-border bg-card p-6 shadow-xs">
            <h3 className="text-base font-bold text-foreground pb-4 border-b border-border/80">
              {isRu ? "Ваш заказ" : "Order Summary"}
            </h3>

            {/* Items list */}
            <div className="mt-4 max-h-72 overflow-y-auto divide-y divide-border/60 pr-1">
              {cart.lineItems.map((item) => (
                <div key={item.id} className="py-3 flex items-center justify-between gap-3 text-xs">
                  <div className="flex-1 min-w-0">
                    <span className="font-semibold text-foreground truncate block">
                      {item.title}
                    </span>
                    <span className="text-muted-foreground block">
                      {item.quantity} x{" "}
                      {parseFloat(item.unitPrice || "0").toFixed(2)} {currencyCode}
                    </span>
                  </div>
                  <span className="font-bold text-foreground shrink-0">
                    {parseFloat(item.totalPrice || "0").toFixed(2)} {currencyCode}
                  </span>
                </div>
              ))}
            </div>

            {/* Calculations breakdown */}
            <div className="mt-6 border-t border-border pt-4 space-y-2.5 text-xs">
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
                  {shippingAmount === 0
                    ? isRu
                      ? "Бесплатно"
                      : "Free"
                    : `${shippingAmount.toFixed(2)} ${currencyCode}`}
                </span>
              </div>

              <div className="flex justify-between border-t border-border pt-3 text-sm font-extrabold text-foreground">
                <span>{isRu ? "Итого к оплате" : "Total Amount"}</span>
                <span className="text-lg text-primary">
                  {grandTotal.toFixed(2)} {currencyCode}
                </span>
              </div>
            </div>

            {/* Submit Button */}
            <div className="mt-6 pt-2">
              <button
                type="submit"
                disabled={isSubmitting}
                className="w-full inline-flex h-12 items-center justify-center gap-2 rounded-xl bg-primary text-sm font-bold text-primary-foreground hover:bg-primary/95 transition shadow-sm disabled:cursor-not-allowed disabled:opacity-60 cursor-pointer"
              >
                {isSubmitting ? (
                  <>
                    <Loader2 className="h-4 w-4 animate-spin" />
                    <span>{isRu ? "Оформление заказа..." : "Processing..."}</span>
                  </>
                ) : (
                  <span>
                    {isRu
                      ? `Подтвердить заказ (${grandTotal.toFixed(2)} ${currencyCode})`
                      : `Place Order (${grandTotal.toFixed(2)} ${currencyCode})`}
                  </span>
                )}
              </button>
            </div>

            {/* Trust Badges */}
            <div className="mt-6 pt-4 border-t border-border/60 space-y-2 text-[11px] text-muted-foreground">
              <div className="flex items-center gap-2">
                <ShieldCheck className="h-4 w-4 text-emerald-500 shrink-0" />
                <span>
                  {isRu
                    ? "Гарантия защиты покупателя и безопасной оплаты"
                    : "100% Secure Checkout & Buyer Protection"}
                </span>
              </div>
              <div className="flex items-center gap-2">
                <Package className="h-4 w-4 text-primary shrink-0" />
                <span>
                  {isRu
                    ? "Возврат или обмен товара в течение 14 дней"
                    : "14-day easy returns and exchanges"}
                </span>
              </div>
            </div>
          </div>
        </div>
      </form>
    </div>
  );
}
