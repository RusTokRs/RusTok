/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { registerStorefrontModule } from "@/modules/registry";
import { CatalogSection } from "./components/catalog-section";

export type {
  ProductCatalogSearchOption,
  ProductCatalogSearchOptions,
  StorefrontCatalogFilter,
  StorefrontEffectivePrice,
  StorefrontProductDetail,
  StorefrontProductListItem,
  StorefrontProductListResponse,
  StorefrontProductPrice,
  StorefrontProductTranslation,
  StorefrontProductVariant,
} from "./api/types";

export {
  fetchCatalogSearchOptions,
  fetchStorefrontProduct,
  fetchStorefrontProductPricing,
  fetchStorefrontProducts,
} from "./api/products";

export { CatalogSection } from "./components/catalog-section";
export { ProductCard } from "./components/product-card";
export { ProductDetailView } from "./components/product-detail-view";
export { ProductFilters } from "./components/product-filters";
export { ProductGrid } from "./components/product-grid";

// Register module in storefront slot: home:afterHero
registerStorefrontModule({
  id: "product-featured-catalog",
  moduleSlug: "product",
  slot: "home:afterHero",
  order: 10,
  render: ({ locale, tenantSlug, searchParams }) => (
    <CatalogSection
      locale={locale}
      tenantSlug={tenantSlug}
      searchParams={searchParams}
    />
  ),
});
