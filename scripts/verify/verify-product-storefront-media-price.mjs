#!/usr/bin/env node
// RusTok product storefront media/price chain guardrails.
//
// Locks the owner → commerce GraphQL → Leptos storefront → Next storefront
// chain that carries the catalog-card image and "from" price, plus the FFA
// attribute-value mutation idempotency argument. Fast source-level checks.

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");

const failures = [];

function readRepo(relativePath) {
  try {
    return readFileSync(path.join(repoRoot, relativePath), "utf8");
  } catch (error) {
    failures.push(`${relativePath}: expected readable source (${error.message})`);
    return "";
  }
}

function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) {
    failures.push(description);
  }
}

function assertMatch(text, pattern, description) {
  if (!pattern.test(text)) failures.push(description);
}

function assertNotContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (found) {
    failures.push(description);
  }
}

const ownerTypesPath = "crates/modules/rustok-product/src/services/catalog/types.rs";
const ownerQueriesPath = "crates/modules/rustok-product/src/services/catalog/queries.rs";
const commerceTypesPath = "crates/modules/rustok-commerce/src/graphql/types.rs";
const commerceCatalogPath = "crates/modules/rustok-commerce/src/graphql/product_catalog.rs";
const leptosModelPath = "crates/modules/rustok-product/storefront/src/model.rs";
const leptosCorePath = "crates/modules/rustok-product/storefront/src/core.rs";
const leptosUiPath = "crates/modules/rustok-product/storefront/src/ui/leptos.rs";
const leptosGraphqlPath =
  "crates/modules/rustok-product/storefront/src/transport/graphql_adapter.rs";
const leptosNativePath =
  "crates/modules/rustok-product/storefront/src/transport/native_server_adapter.rs";
const nextApiPath = "apps/next-frontend/packages/rustok-product/src/api/products.ts";
const nextTypesPath = "apps/next-frontend/packages/rustok-product/src/api/types.ts";
const nextCardPath =
  "apps/next-frontend/packages/rustok-product/src/components/product-card.tsx";
const nextDetailPath =
  "apps/next-frontend/packages/rustok-product/src/components/product-detail-view.tsx";
const ffaProductsPath = "apps/next-admin/packages/rustok-product/src/api/products.ts";
const ffaEditorPath = "apps/next-admin/packages/rustok-product/src/pages/product-editor-page.tsx";
const ffaActionsPath = "apps/next-admin/src/app/dashboard/product/actions.ts";
const productPortPath = "crates/modules/rustok-product/src/media_asset_read_port.rs";
const productRuntimePath = "crates/modules/rustok-product/src/runtime.rs";
const hostAdapterPath = "apps/server/src/services/product_media_asset_validation.rs";
const hostCompositionPath = "apps/server/src/services/commerce_provider_runtime.rs";

const ownerTypes = readRepo(ownerTypesPath);
const ownerQueries = readRepo(ownerQueriesPath);
const commerceTypes = readRepo(commerceTypesPath);
const commerceCatalog = readRepo(commerceCatalogPath);
const leptosModel = readRepo(leptosModelPath);
const leptosCore = readRepo(leptosCorePath);
const leptosUi = readRepo(leptosUiPath);
const leptosGraphql = readRepo(leptosGraphqlPath);
const leptosNative = readRepo(leptosNativePath);
const nextApi = readRepo(nextApiPath);
const nextTypes = readRepo(nextTypesPath);
const nextCard = readRepo(nextCardPath);
const nextDetail = readRepo(nextDetailPath);
const ffaProducts = readRepo(ffaProductsPath);
const ffaEditor = readRepo(ffaEditorPath);
const ffaActions = readRepo(ffaActionsPath);
const productPort = readRepo(productPortPath);
const productRuntime = readRepo(productRuntimePath);
const hostAdapter = readRepo(hostAdapterPath);
const hostComposition = readRepo(hostCompositionPath);

// 1. Owner contract exposes media and price snapshots on list items.
assertContains(
  ownerTypes,
  "pub struct StorefrontProductListImage {",
  `${ownerTypesPath}: storefront list must own a media summary type`
);
assertContains(
  ownerTypes,
  "pub struct StorefrontProductListPrice {",
  `${ownerTypesPath}: storefront list must own a price summary type`
);
assertContains(
  ownerTypes,
  "pub primary_image: Option<StorefrontProductListImage>,",
  `${ownerTypesPath}: list item must expose primary_image`
);
assertContains(
  ownerTypes,
  "pub price_from: Option<StorefrontProductListPrice>,",
  `${ownerTypesPath}: list item must expose price_from`
);
assertContains(
  ownerTypes,
  "pub currency_code: Option<String>,",
  `${ownerTypesPath}: list query must accept a display currency`
);
assertContains(
  ownerTypes,
  "pub fn with_currency_code(",
  `${ownerTypesPath}: list query currency must be validated by a builder`
);

// 2. Owner query batches media and base prices for the page.
assertContains(
  ownerQueries,
  "load_storefront_product_list_media",
  `${ownerQueriesPath}: published list must batch-load catalog-card media`
);
assertContains(
  ownerQueries,
  "fn build_storefront_list_price_from_map(",
  `${ownerQueriesPath}: published list must derive the "from" price`
);
assertContains(
  ownerQueries,
  "price.min_quantity.is_some()",
  `${ownerQueriesPath}: quantity tiers must stay out of the catalog-card snapshot`
);
assertContains(
  ownerQueries,
  "price_list_id.is_some()",
  `${ownerQueriesPath}: price-list rows must stay out of the catalog-card snapshot`
);
assertContains(
  ownerQueries,
  "primary_image: media_summaries.get(&product.id).cloned(),",
  `${ownerQueriesPath}: list item must receive the batched media summary`
);
assertContains(
  ownerQueries,
  "price_from: price_from_by_product.get(&product.id).cloned(),",
  `${ownerQueriesPath}: list item must receive the batched price summary`
);

// 3. Commerce GraphQL exposes both fields on the storefront catalog item.
assertContains(
  commerceTypes,
  "pub struct GqlProductListImage {",
  `${commerceTypesPath}: GraphQL must expose the catalog-card image`
);
assertContains(
  commerceTypes,
  "pub struct GqlProductListPrice {",
  `${commerceTypesPath}: GraphQL must expose the catalog-card price`
);
assertContains(
  commerceTypes,
  "pub primary_image: Option<GqlProductListImage>,",
  `${commerceTypesPath}: GqlProductListItem must expose primary_image`
);
assertContains(
  commerceTypes,
  "pub price_from: Option<GqlProductListPrice>,",
  `${commerceTypesPath}: GqlProductListItem must expose price_from`
);
assertContains(
  commerceTypes,
  "impl From<rustok_product::StorefrontProductListImage> for GqlProductListImage",
  `${commerceTypesPath}: owner media summary must map into the GraphQL type`
);
assertContains(
  commerceTypes,
  "impl From<rustok_product::StorefrontProductListPrice> for GqlProductListPrice",
  `${commerceTypesPath}: owner price summary must map into the GraphQL type`
);
assertContains(
  commerceCatalog,
  "primary_image: item.primary_image.map(Into::into),",
  `${commerceCatalogPath}: storefront catalog must map the owner media summary`
);
assertContains(
  commerceCatalog,
  "price_from: item.price_from.map(Into::into),",
  `${commerceCatalogPath}: storefront catalog must map the owner price summary`
);
assertContains(
  commerceCatalog,
  "pub currency_code: Option<String>,",
  `${commerceCatalogPath}: storefront catalog filter must accept a display currency`
);
assertContains(
  commerceCatalog,
  ".with_currency_code(filter.currency_code)",
  `${commerceCatalogPath}: storefront catalog filter currency must reach the owner query`
);

// 4. Leptos storefront requests and renders media and price.
assertContains(
  leptosModel,
  "pub primary_image: Option<ProductImage>,",
  `${leptosModelPath}: storefront list item must model the primary image`
);
assertContains(
  leptosModel,
  "pub price_from: Option<ProductListPrice>,",
  `${leptosModelPath}: storefront list item must model the "from" price`
);
assertContains(
  leptosModel,
  "pub images: Vec<ProductImage>,",
  `${leptosModelPath}: storefront detail must model the gallery`
);
assertContains(
  leptosGraphql,
  "primaryImage { mediaId url altText position }",
  `${leptosGraphqlPath}: storefront catalog query must request primaryImage`
);
assertContains(
  leptosGraphql,
  "priceFrom { currencyCode amount compareAtAmount onSale }",
  `${leptosGraphqlPath}: storefront catalog query must request priceFrom`
);
assertContains(
  leptosGraphql,
  "images { mediaId url altText position }",
  `${leptosGraphqlPath}: storefront detail query must request images`
);
assertContains(
  leptosNative,
  "map_owner_product_image",
  `${leptosNativePath}: native storefront path must map the owner media summary`
);
assertContains(
  leptosNative,
  "images: value",
  `${leptosNativePath}: native storefront detail must map the gallery`
);
assertContains(
  leptosCore,
  "pub fn format_product_list_price_from(",
  `${leptosCorePath}: storefront must format the "from" price label`
);
assertContains(
  leptosCore,
  "pub gallery: Vec<SelectedProductImageViewModel>,",
  `${leptosCorePath}: selected product view model must carry the gallery`
);
assertContains(
  leptosUi,
  "<img class=\"h-full w-full object-cover\" src=image_url alt=image_alt loading=\"lazy\" />",
  `${leptosUiPath}: catalog card must render the product image`
);
assertContains(
  leptosUi,
  "src=primary.url alt=primary.alt_text",
  `${leptosUiPath}: detail panel must render the primary gallery image`
);
assertContains(
  leptosUi,
  "{price_label}",
  `${leptosUiPath}: catalog card must render the price label`
);

// 5. Next storefront requests and renders media and price.
assertContains(
  nextApi,
  "primaryImage {",
  `${nextApiPath}: storefront catalog query must request primaryImage`
);
assertContains(
  nextApi,
  "priceFrom {",
  `${nextApiPath}: storefront catalog query must request priceFrom`
);
assertContains(
  nextApi,
  "images {",
  `${nextApiPath}: storefront detail query must request images`
);
assertContains(
  nextTypes,
  "primaryImage?: StorefrontProductListImage | null;",
  `${nextTypesPath}: list item must model the primary image`
);
assertContains(
  nextTypes,
  "priceFrom?: StorefrontProductListPrice | null;",
  `${nextTypesPath}: list item must model the "from" price`
);
assertContains(
  nextTypes,
  "images: StorefrontProductImage[];",
  `${nextTypesPath}: detail must model the gallery`
);
assertContains(
  nextCard,
  "product.primaryImage?.url",
  `${nextCardPath}: catalog card must render the owner-resolved image`
);
assertContains(
  nextCard,
  "product.priceFrom.amount",
  `${nextCardPath}: catalog card must render the owner-resolved price`
);
assertNotContains(
  nextCard,
  "Top Media / Thumbnail Placeholder",
  `${nextCardPath}: catalog card placeholder comment must be retired`
);
assertContains(
  nextDetail,
  "const primaryImage = gallery[0] ?? null;",
  `${nextDetailPath}: detail view must derive the primary gallery image`
);
assertContains(
  nextDetail,
  "setActiveImageIndex(index)",
  `${nextDetailPath}: detail view must allow switching gallery images`
);

// 6. FFA attribute-value mutation keeps the mandatory idempotency key.
assertContains(
  ffaProducts,
  "mutation ProductAdminSaveAttributeValues($idempotencyKey: String!",
  `${ffaProductsPath}: save-attribute-values mutation must require $idempotencyKey`
);
assertContains(
  ffaProducts,
  "saveProductAttributeValues(idempotencyKey: $idempotencyKey,",
  `${ffaProductsPath}: save-attribute-values call must pass the idempotency key`
);
assertContains(
  ffaProducts,
  "const idempotencyKey = crypto.randomUUID();\n  const executor = opts.graphql ?? graphqlRequest;",
  `${ffaProductsPath}: save-attribute-values must generate a fresh idempotency key`
);
assertContains(
  ffaActions,
  "attribute values could not be saved",
  `${ffaActionsPath}: partial attribute failure must be reported, not swallowed`
);
assertContains(
  ffaEditor,
  "Changes saved successfully.",
  `${ffaEditorPath}: editor save path must remain the single success surface`
);

// 7. Product validates canonical Media asset references through a host-composed owner port.
assertContains(
  productPort,
  "pub trait ProductMediaAssetReadPort: Send + Sync {",
  `${productPortPath}: Product must own its consumer boundary for Media asset references`
);
assertContains(
  productPort,
  "pub struct ProductMediaValidatedCommandPort {",
  `${productPortPath}: the command port must validate media references before writes`
);
assertContains(
  productPort,
  "self.ensure_media_asset(&context, product_id, input.media_id)",
  `${productPortPath}: add_product_image must validate the new media reference first`
);
assertNotContains(
  productPort,
  "rustok_media",
  `${productPortPath}: Product must not import Media persistence or Media crates`
);
assertContains(
  productPort,
  "product.media_asset_not_found",
  `${productPortPath}: an unknown asset must fail as a typed caller validation error`
);
assertContains(
  productRuntime,
  "pub fn with_media_asset_read_port(",
  `${productRuntimePath}: the host-composed command runtime must accept the Media provider`
);
assertContains(
  productRuntime,
  "ProductMediaValidatedCommandPort::new(",
  `${productRuntimePath}: composing the provider must decorate the command port`
);
assertContains(
  productRuntime,
  "pub const fn media_reference_policy(&self) -> ProductMediaReferencePolicy {",
  `${productRuntimePath}: the runtime must state its media reference policy`
);
assertContains(
  hostAdapter,
  "impl ProductMediaAssetValidationPort for ProductMediaAssetProvider {",
  `${hostAdapterPath}: the host must adapt the Media-owned read port for Product`
);
assertContains(
  hostAdapter,
  ".get_asset(context, media_id)",
  `${hostAdapterPath}: asset validation must resolve through the Media-owned read port`
);
assertContains(
  hostAdapter,
  "if runtime.media_asset_read_port().is_some() {",
  `${hostAdapterPath}: an outer host composition must stay authoritative`
);
// rustfmt wraps the binding as soon as the call grows, so the rule is the composition itself.
assertMatch(
  hostComposition,
  /let runtime =\s*compose_product_catalog_media_asset_validation\(runtime, server, &host\);/,
  `${hostCompositionPath}: Product command composition must attach media asset validation`
);
assertContains(
  hostComposition,
  "crate::services::product_media_asset_validation::compose_product_media_asset_validation(",
  `${hostCompositionPath}: the composition must reuse the host Media adapter`
);

if (failures.length > 0) {
  console.error("[verify-product-storefront-media-price] failures:");
  for (const failure of failures) {
    console.error(`  - ${failure}`);
  }
  process.exit(1);
}

console.log(
  "[verify-product-storefront-media-price] Product storefront media/price chain, FFA attribute idempotency, and catalog-card rendering are source-locked"
);
