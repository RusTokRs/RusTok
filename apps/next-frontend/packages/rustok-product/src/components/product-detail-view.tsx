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
import { ArrowLeft, Boxes, Package, Sparkles } from "lucide-react";
import { Button } from "@/shared/ui/base/button";
import {
  ProductBuyerPanel,
  type ProductBuyerPanelProps,
} from "./product-buyer-panel";
import {
  buildProductSpecificationLabels,
  buildProductSpecifications,
} from "../catalog/specifications";

export type ProductDetailViewProps = ProductBuyerPanelProps;

export function ProductDetailView({
  product,
  locale,
  ...buyerProps
}: ProductDetailViewProps) {
  const isRu = locale === "ru";
  const isBundle =
    product.productType?.toLowerCase() === "bundle" ||
    product.tags.some((tag) => tag.toLowerCase() === "bundle");
  const [activeImageIndex, setActiveImageIndex] = useState(0);

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
    [product.images],
  );
  const primaryImage = gallery[0] ?? null;
  const activeImage = gallery[activeImageIndex] ?? primaryImage;

  // Active translation
  const translation = useMemo(() => {
    return (
      product.translations.find((t) => t.locale === locale) ||
      product.translations[0]
    );
  }, [product.translations, locale]);

  const displayTitle =
    translation?.title || product.handle || (isRu ? "Товар" : "Product");

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
                  activeImage?.altText ||
                  primaryImage.altText ||
                  (isRu
                    ? `Изображение: ${displayTitle}`
                    : `Product image: ${displayTitle}`)
                }
                className="h-full w-full object-cover"
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
            </div>
          </div>

          {gallery.length > 1 && (
            <div className="grid grid-cols-4 gap-2">
              {gallery.slice(0, 8).map((image, index) => (
                <Button
                  key={image.mediaId}
                  variant="ghost"
                  type="button"
                  onClick={() => setActiveImageIndex(index)}
                  aria-label={
                    image.altText ||
                    (isRu
                      ? `Показать изображение ${index + 1}`
                      : `Show image ${index + 1}`)
                  }
                  className={`overflow-hidden rounded-xl border transition ${
                    index === activeImageIndex
                      ? "border-primary ring-2 ring-primary/30"
                      : "border-border hover:border-primary/40"
                  }`}
                >
                  {/* eslint-disable-next-line @next/next/no-img-element */}
                  <img
                    src={image.url}
                    alt={image.altText || displayTitle}
                    loading="lazy"
                    className="aspect-square h-full w-full object-cover"
                  />
                </Button>
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
          </div>

          <ProductBuyerPanel
            {...buyerProps}
            product={product}
            locale={locale}
          />

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
