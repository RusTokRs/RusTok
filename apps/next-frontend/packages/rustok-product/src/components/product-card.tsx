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
import { ArrowRight, Boxes, Package, Sparkles } from "lucide-react";
import type { StorefrontProductListItem } from "../api/types";

interface ProductCardProps {
  product: StorefrontProductListItem;
  locale: string;
}

export function ProductCard({ product, locale }: ProductCardProps) {
  const isBundle =
    product.productType?.toLowerCase() === "bundle" ||
    product.tags.some((t) => t.toLowerCase() === "bundle");
  const isRu = locale === "ru";
  const productUrl = `/${locale}/products/${product.handle}`;

  return (
    <div className="group relative flex flex-col justify-between overflow-hidden rounded-2xl border border-border bg-card p-5 shadow-xs transition-all duration-200 hover:-translate-y-1 hover:border-primary/40 hover:shadow-md">
      <div>
        {/* Top Media: owner-resolved product image with placeholder fallback */}
        <div className="relative mb-4 flex aspect-4/3 w-full items-center justify-center overflow-hidden rounded-xl bg-muted/40 transition-colors group-hover:bg-muted/60">
          {product.primaryImage?.url ? (
            // eslint-disable-next-line @next/next/no-img-element
            <img
              src={product.primaryImage.url}
              alt={
                product.primaryImage.altText ||
                (isRu ? `Изображение: ${product.title}` : `Product image: ${product.title}`)
              }
              loading='lazy'
              className='h-full w-full object-cover transition-transform duration-200 group-hover:scale-105'
            />
          ) : isBundle ? (
            <Boxes className="h-12 w-12 text-primary/70 transition-transform duration-200 group-hover:scale-110" />
          ) : (
            <Package className="h-12 w-12 text-muted-foreground/60 transition-transform duration-200 group-hover:scale-110" />
          )}

          {/* Badges Container */}
          <div className="absolute top-2.5 left-2.5 flex flex-wrap gap-1.5">
            {isBundle && (
              <span className="inline-flex items-center gap-1 rounded-full bg-primary/15 px-2.5 py-0.5 text-[11px] font-semibold tracking-wide text-primary backdrop-blur-xs">
                <Sparkles className="h-3 w-3" />
                {isRu ? "Комплект" : "Bundle"}
              </span>
            )}
            {product.priceFrom?.onSale && (
              <span className="inline-flex items-center rounded-full bg-destructive/85 px-2.5 py-0.5 text-[11px] font-semibold text-destructive-foreground backdrop-blur-xs">
                {isRu ? "Скидка" : "Sale"}
              </span>
            )}
            {product.vendor && (
              <span className="inline-flex items-center rounded-full bg-background/80 px-2 py-0.5 text-[10px] font-medium text-foreground backdrop-blur-xs">
                {product.vendor}
              </span>
            )}
          </div>
        </div>

        {/* Content */}
        <div className="space-y-1.5">
          <h3 className="line-clamp-2 text-base font-semibold tracking-tight text-foreground transition-colors group-hover:text-primary">
            <Link href={productUrl}>{product.title}</Link>
          </h3>

          {product.priceFrom ? (
            <div className="flex items-baseline gap-1.5 pt-0.5">
              <span className="text-sm font-semibold text-foreground">
                {isRu ? 'от ' : 'from '}
                {product.priceFrom.amount} {product.priceFrom.currencyCode}
              </span>
              {product.priceFrom.onSale && product.priceFrom.compareAtAmount && (
                <span className="text-xs text-muted-foreground line-through">
                  {product.priceFrom.compareAtAmount}
                </span>
              )}
            </div>
          ) : (
            <p className="pt-0.5 text-xs text-muted-foreground">
              {isRu ? 'Цена по запросу' : 'Price on request'}
            </p>
          )}

          {product.tags.length > 0 && (
            <div className="flex flex-wrap gap-1 pt-1">
              {product.tags.slice(0, 3).map((tag) => (
                <span
                  key={tag}
                  className="rounded-md bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground"
                >
                  #{tag}
                </span>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* Footer / CTA */}
      <div className="mt-5 flex items-center justify-between border-t border-border/60 pt-3">
        <span className="text-xs text-muted-foreground">
          {isRu ? "Подробнее о товаре" : "View specifications"}
        </span>
        <Link
          href={productUrl}
          className="inline-flex items-center gap-1 text-xs font-semibold text-primary transition-all group-hover:gap-1.5 hover:underline"
        >
          {isRu ? "Открыть" : "View"}
          <ArrowRight className="h-3.5 w-3.5" />
        </Link>
      </div>
    </div>
  );
}
