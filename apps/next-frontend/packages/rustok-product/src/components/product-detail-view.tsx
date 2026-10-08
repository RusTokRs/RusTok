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
  Boxes,
  Check,
  CheckCircle2,
  Loader2,
  Minus,
  Package,
  Plus,
  ShoppingCart,
  Sparkles,
} from "lucide-react";
import { useCart } from "@rustok/cart-frontend";
import type {
  StorefrontProductDetail,
  StorefrontProductVariant,
} from "../api/types";
import {
  buildProductSpecificationLabels,
  buildProductSpecifications,
} from "../catalog/specifications";

interface ProductDetailViewProps {
  product: StorefrontProductDetail;
  pricingVariants?: StorefrontProductVariant[];
  locale: string;
}

export function ProductDetailView({
  product,
  pricingVariants = [],
  locale,
}: ProductDetailViewProps) {
  const isRu = locale === "ru";
  const isBundle =
    product.productType?.toLowerCase() === "bundle" ||
    product.tags.some((t) => t.toLowerCase() === "bundle");

  // Merge variants with pricing details if available
  const variants = useMemo(() => {
    return product.variants.map((v) => {
      const pricing = pricingVariants.find((pv) => pv.id === v.id || pv.sku === v.sku);
      return {
        ...v,
        effectivePrice: pricing?.effectivePrice || v.effectivePrice,
        prices: pricing?.prices?.length ? pricing.prices : v.prices,
      };
    });
  }, [product.variants, pricingVariants]);

  const { addItem, openCart } = useCart();

  // Selected variant state
  const [selectedVariantId, setSelectedVariantId] = useState<string>(
    variants[0]?.id || ""
  );
  const [quantity, setQuantity] = useState<number>(1);
  const [isAdding, setIsAdding] = useState<boolean>(false);
  const [isAdded, setIsAdded] = useState<boolean>(false);
  const [activeImageIndex, setActiveImageIndex] = useState<number>(0);

  // Owner-resolved specifications: localized labels and values, service attributes already
  // filtered out by the Product module; the storefront only phrases the boolean vocabulary.
  const specificationLabels = useMemo(
    () => buildProductSpecificationLabels(locale),
    [locale],
  );
  const specifications = useMemo(
    () => buildProductSpecifications(product, specificationLabels),
    [product, specificationLabels],
  );

  // Owner-resolved product gallery (locale-resolved by the Product module).
  const gallery = useMemo(
    () => [...(product.images ?? [])].sort((a, b) => a.position - b.position),
    [product.images]
  );
  const primaryImage = gallery[0] ?? null;
  const activeImage = gallery[activeImageIndex] ?? primaryImage;

  const selectedVariant = useMemo(() => {
    return variants.find((v) => v.id === selectedVariantId) || variants[0];
  }, [variants, selectedVariantId]);

  // Active translation
  const translation = useMemo(() => {
    return (
      product.translations.find((t) => t.locale === locale) ||
      product.translations[0]
    );
  }, [product.translations, locale]);

  const displayTitle =
    translation?.title || product.handle || (isRu ? "Товар" : "Product");

  // Price calculations
  const priceInfo = useMemo(() => {
    if (!selectedVariant) return null;
    const effective = selectedVariant.effectivePrice;
    if (effective) {
      return {
        amount: effective.amount,
        compareAt: effective.compareAtAmount,
        currency: effective.currencyCode,
        discountPercent: effective.discountPercent,
        onSale: effective.onSale,
      };
    }
    const standard = selectedVariant.prices[0];
    if (standard) {
      return {
        amount: standard.amount,
        compareAt: standard.compareAtAmount,
        currency: standard.currencyCode,
        discountPercent: standard.discountPercent,
        onSale: standard.onSale,
      };
    }
    return null;
  }, [selectedVariant]);

  const handleAddToCart = async () => {
    if (!selectedVariant) return;
    setIsAdding(true);
    try {
      const ok = await addItem(selectedVariant.id, quantity);
      if (ok) {
        setIsAdded(true);
        setTimeout(() => setIsAdded(false), 2000);
        openCart();
      }
    } finally {
      setIsAdding(false);
    }
  };

  const inStock = selectedVariant ? selectedVariant.inStock : false;
  const stockQty = selectedVariant?.inventoryQuantity ?? null;

  return (
    <div className="space-y-8 pb-12 animate-in fade-in duration-200">
      {/* Back button */}
      <div>
        <Link
          href={`/${locale}/products`}
          className="inline-flex items-center gap-1.5 text-xs font-semibold text-muted-foreground hover:text-foreground transition"
        >
          <ArrowLeft className="h-3.5 w-3.5" />
          {isRu ? "Назад в каталог" : "Back to catalog"}
        </Link>
      </div>

      {/* Main 2-Column Product Layout */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-8 items-start">
        {/* Left Column: Media Gallery / Image Showcase (5 cols) */}
        <div className="lg:col-span-5 space-y-4">
          <div className="relative aspect-square w-full flex items-center justify-center rounded-3xl border border-border bg-card p-8 shadow-xs overflow-hidden">
            {primaryImage ? (
              // eslint-disable-next-line @next/next/no-img-element
              <img
                src={activeImage?.url || primaryImage.url}
                alt={
                  (activeImage?.altText || primaryImage.altText) ||
                  (isRu ? `Изображение: ${displayTitle}` : `Product image: ${displayTitle}`)
                }
                className='h-full w-full object-cover'
              />
            ) : isBundle ? (
              <Boxes className="h-32 w-32 text-primary/70" />
            ) : (
              <Package className="h-32 w-32 text-muted-foreground/60" />
            )}

            {/* Badges */}
            <div className="absolute top-4 left-4 flex flex-col gap-1.5">
              {isBundle && (
                <span className="inline-flex items-center gap-1 rounded-full bg-primary px-3 py-1 text-xs font-bold text-primary-foreground shadow-xs">
                  <Sparkles className="h-3.5 w-3.5" />
                  {isRu ? "Комплект" : "Bundle"}
                </span>
              )}
              {priceInfo?.onSale && (
                <span className="inline-flex items-center rounded-full bg-destructive px-3 py-1 text-xs font-bold text-destructive-foreground shadow-xs">
                  {priceInfo.discountPercent
                    ? `-${priceInfo.discountPercent}%`
                    : isRu
                    ? "Скидка"
                    : "Sale"}
                </span>
              )}
            </div>
          </div>

          {gallery.length > 1 && (
            <div className="grid grid-cols-4 gap-2">
              {gallery.slice(0, 8).map((image, index) => (
                <button
                  key={image.mediaId}
                  type='button'
                  onClick={() => setActiveImageIndex(index)}
                  aria-label={
                    image.altText ||
                    (isRu
                      ? `Показать изображение ${index + 1}`
                      : `Show image ${index + 1}`)
                  }
                  className={`overflow-hidden rounded-xl border transition ${
                    index === activeImageIndex
                      ? 'border-primary ring-2 ring-primary/30'
                      : 'border-border hover:border-primary/40'
                  }`}
                >
                  {/* eslint-disable-next-line @next/next/no-img-element */}
                  <img
                    src={image.url}
                    alt={image.altText || displayTitle}
                    loading='lazy'
                    className='aspect-square h-full w-full object-cover'
                  />
                </button>
              ))}
            </div>
          )}

          {/* Specifications filled by the Product owner (localized labels and values). */}
          {specifications.length > 0 && (
            <section className="rounded-2xl border border-border bg-card p-4 text-xs">
              <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                {specificationLabels.title}
              </h2>
              <dl className="mt-3 divide-y divide-border">
                {specifications.map((row) => (
                  <div
                    key={row.code}
                    className="flex items-baseline justify-between gap-4 py-2"
                  >
                    <dt className="text-muted-foreground">{row.label}</dt>
                    <dd className="text-right font-medium text-foreground">
                      {row.value}
                    </dd>
                  </div>
                ))}
              </dl>
            </section>
          )}
        </div>

        {/* Right Column: Details, Variants & Purchase Actions (7 cols) */}
        <div className="lg:col-span-7 space-y-6">
          <div className="space-y-2 border-b border-border pb-5">
            {product.vendor && (
              <p className="text-xs font-semibold uppercase tracking-wider text-primary">
                {product.vendor}
              </p>
            )}
            <h1 className="text-2xl sm:text-3xl font-bold tracking-tight text-foreground">
              {translation?.title || product.handle}
            </h1>
            {selectedVariant && (
              <p className="text-xs text-muted-foreground">
                {isRu ? "Артикул: " : "SKU: "}
                <span className="font-mono text-foreground">{selectedVariant.sku}</span>
              </p>
            )}
          </div>

          {/* Price Block */}
          {priceInfo ? (
            <div className="flex items-baseline gap-3">
              <span className="text-3xl font-extrabold tracking-tight text-foreground">
                {priceInfo.amount.toFixed(2)} {priceInfo.currency}
              </span>
              {priceInfo.compareAt && priceInfo.compareAt > priceInfo.amount && (
                <span className="text-lg text-muted-foreground line-through">
                  {priceInfo.compareAt.toFixed(2)} {priceInfo.currency}
                </span>
              )}
              {priceInfo.discountPercent && (
                <span className="rounded-full bg-destructive/15 px-2.5 py-0.5 text-xs font-bold text-destructive">
                  -{priceInfo.discountPercent}%
                </span>
              )}
            </div>
          ) : (
            <div className="text-sm text-muted-foreground">
              {isRu ? "Цена по запросу" : "Price on request"}
            </div>
          )}

          {/* Stock Status Badge */}
          <div className="flex items-center gap-2 text-xs">
            {inStock ? (
              <span className="inline-flex items-center gap-1.5 font-medium text-emerald-600 dark:text-emerald-400">
                <CheckCircle2 className="h-4 w-4" />
                {isRu
                  ? stockQty != null
                    ? `В наличии (${stockQty} шт.)`
                    : "В наличии"
                  : stockQty != null
                  ? `In stock (${stockQty} units)`
                  : "In stock"}
              </span>
            ) : (
              <span className="inline-flex items-center gap-1.5 font-medium text-destructive">
                ✕ {isRu ? "Нет в наличии" : "Out of stock"}
              </span>
            )}
          </div>

          {/* Variant Selector */}
          {variants.length > 1 && (
            <div className="space-y-3 border-t border-border pt-5">
              <label className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                {isRu ? "Выберите вариант:" : "Select variant:"}
              </label>
              <div className="flex flex-wrap gap-2">
                {variants.map((v) => {
                  const isSelected = v.id === selectedVariantId;
                  return (
                    <button
                      key={v.id}
                      type="button"
                      onClick={() => setSelectedVariantId(v.id)}
                      className={`h-10 px-4 rounded-xl border text-xs font-medium transition ${
                        isSelected
                          ? "border-primary bg-primary text-primary-foreground shadow-xs"
                          : "border-border bg-card text-foreground hover:bg-accent"
                      } ${!v.inStock ? "opacity-60 line-through" : ""}`}
                    >
                      {v.title || v.sku}
                    </button>
                  );
                })}
              </div>
            </div>
          )}

          {/* Quantity & Add to Cart Controls */}
          <div className="flex flex-col sm:flex-row items-stretch sm:items-center gap-3 pt-2">
            <div className="inline-flex h-11 items-center justify-between rounded-xl border border-border bg-card px-2 min-w-[120px]">
              <button
                type="button"
                onClick={() => setQuantity(Math.max(1, quantity - 1))}
                className="flex h-8 w-8 items-center justify-center rounded-lg text-foreground hover:bg-accent transition"
                aria-label="Decrease quantity"
              >
                <Minus className="h-3.5 w-3.5" />
              </button>
              <span className="font-semibold text-sm text-foreground">{quantity}</span>
              <button
                type="button"
                onClick={() => setQuantity(quantity + 1)}
                className="flex h-8 w-8 items-center justify-center rounded-lg text-foreground hover:bg-accent transition"
                aria-label="Increase quantity"
              >
                <Plus className="h-3.5 w-3.5" />
              </button>
            </div>

            <button
              type="button"
              onClick={() => void handleAddToCart()}
              disabled={!inStock || isAdding}
              className={`flex-1 h-11 inline-flex items-center justify-center gap-2 rounded-xl text-sm font-semibold transition shadow-xs ${
                isAdded
                  ? "bg-emerald-600 text-white"
                  : inStock
                  ? "bg-primary text-primary-foreground hover:bg-primary/90"
                  : "bg-muted text-muted-foreground cursor-not-allowed"
              }`}
            >
              {isAdding ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  {isRu ? "Добавление..." : "Adding..."}
                </>
              ) : isAdded ? (
                <>
                  <Check className="h-4 w-4" />
                  {isRu ? "Добавлено в корзину!" : "Added to cart!"}
                </>
              ) : (
                <>
                  <ShoppingCart className="h-4 w-4" />
                  {isRu ? "Добавить в корзину" : "Add to Cart"}
                </>
              )}
            </button>
          </div>

          {/* Description Section */}
          {translation?.description && (
            <div className="space-y-3 border-t border-border pt-6">
              <h2 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
                {isRu ? "Описание товара" : "Product Description"}
              </h2>
              <div className="prose prose-sm dark:prose-invert max-w-none text-muted-foreground leading-relaxed">
                <p className="whitespace-pre-line">{translation.description}</p>
              </div>
            </div>
          )}

          {/* Tags */}
          {product.tags.length > 0 && (
            <div className="flex flex-wrap gap-1.5 border-t border-border pt-4">
              {product.tags.map((tag) => (
                <span
                  key={tag}
                  className="rounded-lg border border-border bg-card px-2.5 py-1 text-xs text-muted-foreground"
                >
                  #{tag}
                </span>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
