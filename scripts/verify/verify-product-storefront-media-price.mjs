#!/usr/bin/env node
// RusTok product storefront media/price chain guardrails.
//
// Locks the owner → commerce GraphQL → Leptos storefront → Next storefront
// chain that carries the catalog-card image and "from" price, plus the FFA
// attribute-value mutation idempotency argument. Fast source-level checks.

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, '../..');

const failures = [];

function readRepo(relativePath) {
  try {
    return readFileSync(path.join(repoRoot, relativePath), 'utf8');
  } catch (error) {
    failures.push(
      `${relativePath}: expected readable source (${error.message})`
    );
    return '';
  }
}

function assertAbsent(source, text, message) {
  if (source.includes(text)) failures.push(message);
}

function assertContains(text, pattern, description) {
  const found =
    typeof pattern === 'string' ? text.includes(pattern) : pattern.test(text);
  if (!found) {
    failures.push(description);
  }
}

function assertMatch(text, pattern, description) {
  if (!pattern.test(text)) failures.push(description);
}

function assertNotContains(text, pattern, description) {
  const found =
    typeof pattern === 'string' ? text.includes(pattern) : pattern.test(text);
  if (found) {
    failures.push(description);
  }
}

const ownerTypesPath =
  'crates/modules/rustok-product/src/services/catalog/types.rs';
const ownerQueriesPath =
  'crates/modules/rustok-product/src/services/catalog/queries.rs';
const commerceTypesPath = 'crates/modules/rustok-commerce/src/graphql/types.rs';
const commerceCatalogPath =
  'crates/modules/rustok-commerce/src/graphql/product_catalog.rs';
const leptosModelPath = 'crates/modules/rustok-product/storefront/src/model.rs';
const leptosCorePath = 'crates/modules/rustok-product/storefront/src/core.rs';
const leptosUiPath =
  'crates/modules/rustok-product/storefront/src/ui/leptos.rs';
const leptosGraphqlPath =
  'crates/modules/rustok-product/storefront/src/transport/graphql_adapter.rs';
const leptosNativePath =
  'crates/modules/rustok-product/storefront/src/transport/native_server_adapter.rs';
const nextApiPath =
  'apps/next-frontend/packages/rustok-product/src/api/products.ts';
const nextTypesPath =
  'apps/next-frontend/packages/rustok-product/src/api/types.ts';
const nextCardPath =
  'apps/next-frontend/packages/rustok-product/src/components/product-card.tsx';
const nextDetailPath =
  'apps/next-frontend/packages/rustok-product/src/components/product-detail-view.tsx';
const ffaProductsPath =
  'apps/next-admin/packages/rustok-product/src/api/products.ts';
const ffaEditorPath =
  'apps/next-admin/packages/rustok-product/src/pages/product-editor-page.tsx';
const ffaActionsPath = 'apps/next-admin/src/app/dashboard/product/actions.ts';
const productPortPath = 'crates/modules/rustok-product/src/media_reference.rs';
const productRuntimePath = 'crates/modules/rustok-product/src/runtime.rs';
const hostAdapterPath = 'apps/server/src/services/product_media_reference.rs';
const hostCompositionPath =
  'apps/server/src/services/commerce_provider_runtime.rs';

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
  'pub struct StorefrontProductListImage {',
  `${ownerTypesPath}: storefront list must own a media summary type`
);
assertContains(
  ownerTypes,
  'pub struct StorefrontProductListPrice {',
  `${ownerTypesPath}: storefront list must own a price summary type`
);
assertContains(
  ownerTypes,
  'pub primary_image: Option<StorefrontProductListImage>,',
  `${ownerTypesPath}: list item must expose primary_image`
);
assertContains(
  ownerTypes,
  'pub price_from: Option<StorefrontProductListPrice>,',
  `${ownerTypesPath}: list item must expose price_from`
);
assertContains(
  ownerTypes,
  'pub currency_code: Option<String>,',
  `${ownerTypesPath}: list query must accept a display currency`
);
assertContains(
  ownerTypes,
  'pub fn with_currency_code(',
  `${ownerTypesPath}: list query currency must be validated by a builder`
);

// 2. Owner query batches media and base prices for the page.
assertContains(
  ownerQueries,
  'load_storefront_product_list_media',
  `${ownerQueriesPath}: published list must batch-load catalog-card media`
);
assertContains(
  ownerQueries,
  'fn build_storefront_list_price_from_map(',
  `${ownerQueriesPath}: published list must derive the "from" price`
);
assertContains(
  ownerQueries,
  'price.min_quantity.is_some()',
  `${ownerQueriesPath}: quantity tiers must stay out of the catalog-card snapshot`
);
assertContains(
  ownerQueries,
  'price_list_id.is_some()',
  `${ownerQueriesPath}: price-list rows must stay out of the catalog-card snapshot`
);
assertContains(
  ownerQueries,
  'primary_image: media_summaries.get(&product.id).cloned(),',
  `${ownerQueriesPath}: list item must receive the batched media summary`
);
assertContains(
  ownerQueries,
  'price_from: price_from_by_product.get(&product.id).cloned(),',
  `${ownerQueriesPath}: list item must receive the batched price summary`
);

// 3. Commerce GraphQL exposes both fields on the storefront catalog item.
assertContains(
  commerceTypes,
  'pub struct GqlProductListImage {',
  `${commerceTypesPath}: GraphQL must expose the catalog-card image`
);
assertContains(
  commerceTypes,
  'pub struct GqlProductListPrice {',
  `${commerceTypesPath}: GraphQL must expose the catalog-card price`
);
assertContains(
  commerceTypes,
  'pub primary_image: Option<GqlProductListImage>,',
  `${commerceTypesPath}: GqlProductListItem must expose primary_image`
);
assertContains(
  commerceTypes,
  'pub price_from: Option<GqlProductListPrice>,',
  `${commerceTypesPath}: GqlProductListItem must expose price_from`
);
assertContains(
  commerceTypes,
  'impl From<rustok_product::StorefrontProductListImage> for GqlProductListImage',
  `${commerceTypesPath}: owner media summary must map into the GraphQL type`
);
assertContains(
  commerceTypes,
  'impl From<rustok_product::StorefrontProductListPrice> for GqlProductListPrice',
  `${commerceTypesPath}: owner price summary must map into the GraphQL type`
);
assertContains(
  commerceCatalog,
  'primary_image: item.primary_image.map(Into::into),',
  `${commerceCatalogPath}: storefront catalog must map the owner media summary`
);
assertContains(
  commerceCatalog,
  'price_from: item.price_from.map(Into::into),',
  `${commerceCatalogPath}: storefront catalog must map the owner price summary`
);
assertContains(
  commerceCatalog,
  'pub currency_code: Option<String>,',
  `${commerceCatalogPath}: storefront catalog filter must accept a display currency`
);
assertContains(
  commerceCatalog,
  '.with_currency_code(filter.currency_code)',
  `${commerceCatalogPath}: storefront catalog filter currency must reach the owner query`
);

// 4. Leptos storefront requests and renders media and price.
assertContains(
  leptosModel,
  'pub primary_image: Option<ProductImage>,',
  `${leptosModelPath}: storefront list item must model the primary image`
);
assertContains(
  leptosModel,
  'pub price_from: Option<ProductListPrice>,',
  `${leptosModelPath}: storefront list item must model the "from" price`
);
assertContains(
  leptosModel,
  'pub images: Vec<ProductImage>,',
  `${leptosModelPath}: storefront detail must model the gallery`
);
assertContains(
  leptosGraphql,
  'primaryImage { mediaId url altText position }',
  `${leptosGraphqlPath}: storefront catalog query must request primaryImage`
);
assertContains(
  leptosGraphql,
  'priceFrom { currencyCode amount compareAtAmount onSale }',
  `${leptosGraphqlPath}: storefront catalog query must request priceFrom`
);
assertContains(
  leptosGraphql,
  'images { mediaId url altText position }',
  `${leptosGraphqlPath}: storefront detail query must request images`
);
assertContains(
  leptosNative,
  'map_owner_product_image',
  `${leptosNativePath}: native storefront path must map the owner media summary`
);
assertContains(
  leptosNative,
  'images: value',
  `${leptosNativePath}: native storefront detail must map the gallery`
);
assertContains(
  leptosCore,
  'pub fn format_product_list_price_from(',
  `${leptosCorePath}: storefront must format the "from" price label`
);
assertContains(
  leptosCore,
  'pub gallery: Vec<SelectedProductImageViewModel>,',
  `${leptosCorePath}: selected product view model must carry the gallery`
);
assertContains(
  leptosUi,
  '<img class="h-full w-full object-cover" src=image_url alt=image_alt loading="lazy" />',
  `${leptosUiPath}: catalog card must render the product image`
);
assertContains(
  leptosUi,
  'src=primary.url alt=primary.alt_text',
  `${leptosUiPath}: detail panel must render the primary gallery image`
);
assertContains(
  leptosUi,
  '{price_label}',
  `${leptosUiPath}: catalog card must render the price label`
);

// 5. Next storefront requests and renders media and price.
assertContains(
  nextApi,
  'primaryImage {',
  `${nextApiPath}: storefront catalog query must request primaryImage`
);
assertContains(
  nextApi,
  'priceFrom {',
  `${nextApiPath}: storefront catalog query must request priceFrom`
);
assertContains(
  nextApi,
  'images {',
  `${nextApiPath}: storefront detail query must request images`
);
assertContains(
  nextTypes,
  'primaryImage?: StorefrontProductListImage | null;',
  `${nextTypesPath}: list item must model the primary image`
);
assertContains(
  nextTypes,
  'priceFrom?: StorefrontProductListPrice | null;',
  `${nextTypesPath}: list item must model the "from" price`
);
assertContains(
  nextTypes,
  'images: StorefrontProductImage[];',
  `${nextTypesPath}: detail must model the gallery`
);
assertContains(
  nextCard,
  'product.primaryImage?.url',
  `${nextCardPath}: catalog card must render the owner-resolved image`
);
assertContains(
  nextCard,
  'product.priceFrom.amount',
  `${nextCardPath}: catalog card must render the owner-resolved price`
);
assertNotContains(
  nextCard,
  'Top Media / Thumbnail Placeholder',
  `${nextCardPath}: catalog card placeholder comment must be retired`
);
assertContains(
  nextDetail,
  'const primaryImage = gallery[0] ?? null;',
  `${nextDetailPath}: detail view must derive the primary gallery image`
);
assertContains(
  nextDetail,
  'setActiveImageIndex(index)',
  `${nextDetailPath}: detail view must allow switching gallery images`
);

// 6. FFA attribute-value mutation keeps the mandatory idempotency key.
assertContains(
  ffaProducts,
  'mutation ProductAdminSaveAttributeValues($idempotencyKey: String!',
  `${ffaProductsPath}: save-attribute-values mutation must require $idempotencyKey`
);
assertContains(
  ffaProducts,
  'saveProductAttributeValues(idempotencyKey: $idempotencyKey,',
  `${ffaProductsPath}: save-attribute-values call must pass the idempotency key`
);
assertContains(
  ffaProducts,
  'const idempotencyKey = crypto.randomUUID();\n  const executor = opts.graphql ?? graphqlRequest;',
  `${ffaProductsPath}: save-attribute-values must generate a fresh idempotency key`
);
assertContains(
  ffaActions,
  'attributeValues: payload.attributePatches.length',
  `${ffaActionsPath}: values must be composed into the atomic catalog save`
);
assertContains(
  ffaEditor,
  'Changes saved successfully.',
  `${ffaEditorPath}: editor save path must remain the single success surface`
);

assertAbsent(
  ffaActions,
  'await saveProductAttributeValues(',
  `${ffaActionsPath}: editor save must not commit a second attribute command`
);
assertAbsent(
  ffaActions,
  'await deleteProductDetail(opts, created.id)',
  `${ffaActionsPath}: editor save must not compensate a receipted create with deletion`
);

// 7. Product's consumer boundary is installed inside the receipted owner.
assertContains(
  productPort,
  'pub trait ProductMediaReferencePort: Send + Sync {',
  `${productPortPath}: Product must own its retention consumer boundary`
);
assertContains(
  productPort,
  'pub(crate) async fn retain_product_image(',
  `${productPortPath}: image retention must execute inside the owner command`
);
assertNotContains(
  productPort,
  'rustok_media',
  `${productPortPath}: Product must not import Media persistence or Media crates`
);
assertNotContains(
  productRuntime,
  'ProductMediaValidatedCommandPort',
  `${productRuntimePath}: no pre-replay Media decorator is allowed`
);
assertContains(
  productRuntime,
  'pub fn with_media_reference_port(',
  `${productRuntimePath}: command runtime must accept the retention provider`
);
assertContains(
  productRuntime,
  'ProductMediaReferencePolicy::Unavailable',
  `${productRuntimePath}: absent Media capability must fail closed`
);
assertContains(
  hostAdapter,
  'impl ProductMediaReferencePort for ProductMediaReferenceProvider {',
  `${hostAdapterPath}: host must adapt Media durable ports`
);
assertContains(
  hostAdapter,
  '.retain_asset_reference(',
  `${hostAdapterPath}: host must request a durable Media hold`
);
assertMatch(
  hostComposition,
  /let runtime =\s*compose_product_catalog_media_references\(runtime, server, &host\);/,
  `${hostCompositionPath}: Product command composition must attach media retention`
);
assertContains(
  hostComposition,
  'crate::services::product_media_reference::compose_product_media_references(',
  `${hostCompositionPath}: composition must reuse the selected Media adapter`
);

if (failures.length > 0) {
  console.error('[verify-product-storefront-media-price] failures:');
  for (const failure of failures) {
    console.error(`  - ${failure}`);
  }
  process.exit(1);
}

console.log(
  '[verify-product-storefront-media-price] Product storefront media/price chain, FFA attribute idempotency, and catalog-card rendering are source-locked'
);
