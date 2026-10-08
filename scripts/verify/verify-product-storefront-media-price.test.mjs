#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-product-storefront-media-price.mjs");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function ownerTypesSource({ includePriceField = true } = {}) {
  return `
pub struct StorefrontProductListImage { pub media_id: String }
pub struct StorefrontProductListPrice { pub amount: String }
pub struct StorefrontProductListItem {
    pub id: String,
    pub primary_image: Option<StorefrontProductListImage>,
    ${includePriceField ? "pub price_from: Option<StorefrontProductListPrice>," : ""}
}
pub struct StorefrontProductListQuery {
    pub currency_code: Option<String>,
}
impl StorefrontProductListQuery {
    pub fn with_currency_code(self, currency_code: Option<String>) -> Self { self }
}
`;
}

function ownerQueriesSource({ includeMediaBatch = true } = {}) {
  return `
fn build_storefront_list_price_from_map() -> Vec<()> { Vec::new() }
async fn published_list(&self) {
    ${includeMediaBatch ? "let media_summaries = self.load_storefront_product_list_media();" : ""}
    let price_from_by_product = build_storefront_list_price_from_map();
    if price_list_id.is_some() {}
    if price.min_quantity.is_some() {}
    let item = StorefrontProductListItem {
        primary_image: media_summaries.get(&product.id).cloned(),
        price_from: price_from_by_product.get(&product.id).cloned(),
    };
}
`;
}

function commerceTypesSource() {
  return `
pub struct GqlProductListImage { pub media_id: String }
pub struct GqlProductListPrice { pub amount: String }
pub struct GqlProductListItem {
    pub primary_image: Option<GqlProductListImage>,
    pub price_from: Option<GqlProductListPrice>,
}
impl From<rustok_product::StorefrontProductListImage> for GqlProductListImage { fn from(v: rustok_product::StorefrontProductListImage) -> Self { Self { media_id: v.media_id } } }
impl From<rustok_product::StorefrontProductListPrice> for GqlProductListPrice { fn from(v: rustok_product::StorefrontProductListPrice) -> Self { Self { amount: v.amount } } }
`;
}

function commerceCatalogSource({ includeCurrency = true } = {}) {
  return `
pub struct StorefrontProductCatalogFilter {
    pub currency_code: Option<String>,
}
fn map_item(item: StorefrontProductListItem) -> GqlProductListItem {
    GqlProductListItem {
        primary_image: item.primary_image.map(Into::into),
        price_from: item.price_from.map(Into::into),
    }
}
fn build(filter: StorefrontProductCatalogFilter) {
    ${includeCurrency ? ".with_currency_code(filter.currency_code)" : ""}
}
`;
}

function leptosModelSource() {
  return `
pub struct ProductImage { pub media_id: String }
pub struct ProductListPrice { pub amount: String }
pub struct ProductListItem {
    pub primary_image: Option<ProductImage>,
    pub price_from: Option<ProductListPrice>,
}
pub struct ProductDetail {
    pub images: Vec<ProductImage>,
}
`;
}

function leptosCoreSource() {
  return `
pub fn format_product_list_price_from(price: &crate::model::ProductListPrice) -> String { String::new() }
pub struct SelectedProductViewModel { pub gallery: Vec<SelectedProductImageViewModel>, }
pub struct SelectedProductImageViewModel { pub url: String }
`;
}

function leptosUiSource({ includeCardImage = true } = {}) {
  return `
pub fn CatalogRail() {
    ${includeCardImage ? '<img class="h-full w-full object-cover" src=image_url alt=image_alt loading="lazy" />' : ""}
    <img class="h-full w-full object-cover" src=primary.url alt=primary.alt_text />
    <span>{price_label}</span>
}
`;
}

function leptosGraphqlSource() {
  return `
const LIST: &str = "query { storefrontProductCatalog { items { primaryImage { mediaId url altText position } priceFrom { currencyCode amount compareAtAmount onSale } } } }";
const DETAIL: &str = "query { storefrontProduct { images { mediaId url altText position } } }";
`;
}

function leptosNativeSource() {
  return `
fn map_product_list_item(value: StorefrontProductListItem) -> ProductListItem {
    ProductListItem {
        primary_image: value.primary_image.map(super::catalog_list_native::map_owner_product_image),
    }
}
fn map_product_detail(value: StorefrontProductDetail) -> ProductDetail {
    ProductDetail {
        images: value.images.into_iter().map(map_product_image).collect(),
    }
}
`;
}

function nextApiSource() {
  return `
export const STOREFRONT_PRODUCTS_QUERY = \`
  items {
    primaryImage { mediaId }
    priceFrom { currencyCode }
  }
\`;
export const STOREFRONT_PRODUCT_QUERY = \`
  images { mediaId }
\`;
`;
}

function nextTypesSource() {
  return `
export type StorefrontProductListItem = {
  primaryImage?: StorefrontProductListImage | null;
  priceFrom?: StorefrontProductListPrice | null;
};
export type StorefrontProductDetail = {
  images: StorefrontProductImage[];
};
`;
}

function nextCardSource({ includeImage = true } = {}) {
  return `
export function ProductCard({ product, locale }) {
  return (
    <div>
      {${includeImage ? "product.primaryImage?.url ? <img src={product.primaryImage.url} /> : null" : "null"}}
      <span>{product.priceFrom.amount}</span>
    </div>
  );
}
`;
}

function nextDetailSource() {
  return `
export function ProductDetailView({ product }) {
  const gallery = product.images ?? [];
  const primaryImage = gallery[0] ?? null;
  return <button onClick={() => setActiveImageIndex(index)} />;
}
`;
}

function ffaProductsSource({ includeIdempotency = true } = {}) {
  return `
export const SAVE_PRODUCT_ATTRIBUTE_VALUES_MUTATION = \`
  ${includeIdempotency ? "mutation ProductAdminSaveAttributeValues($idempotencyKey: String!," : "mutation ProductAdminSaveAttributeValues("}
    $productId: UUID!
  ) {
    saveProductAttributeValues(${includeIdempotency ? "idempotencyKey: $idempotencyKey," : ""}
      productId: $productId
    ) { id }
  }
\`;
export async function saveProductAttributeValues(opts) {
  ${includeIdempotency ? "const idempotencyKey = crypto.randomUUID();\n  const executor = opts.graphql ?? graphqlRequest;" : "const executor = opts.graphql ?? graphqlRequest;"}
}
`;
}

function ffaEditorSource() {
  return `export function ProductEditorPage() { toast.success("Changes saved successfully."); }`;
}

function ffaActionsSource({ includePartialFailure = true } = {}) {
  return `
export async function saveProductAction() {
  ${includePartialFailure ? 'return { warning: "attribute values could not be saved" };' : "return {};"}
}
`;
}

function productPortSource({ includeValidation = true } = {}) {
  return `
pub const ENSURE_PRODUCT_IMAGE_MEDIA_ASSET_OPERATION: &str = "ensure_product_image_media_asset";
pub struct ProductMediaValidatedCommandPort {
    inner: Arc<dyn ProductCatalogCommandPort>,
    media_asset_read_port: Arc<dyn ProductMediaAssetReadPort>,
}
pub trait ProductMediaAssetReadPort: Send + Sync {
    async fn ensure_asset_exists(&self, context: PortContext, media_id: Uuid) -> Result<(), PortError>;
}
async fn add_product_image(&self, context: PortContext, product_id: Uuid, input: AddProductImageInput) -> Result<ProductImageResponse, PortError> {
    ${includeValidation ? "self.ensure_media_asset(&context, product_id, input.media_id)\n            .await?;" : ""}
    self.inner.add_product_image(context, product_id, input).await
}
fn ensure_media_asset(&self) {
    PortError::validation("product.media_asset_not_found", "product image media asset was not found");
}
`;
}

function productRuntimeSource() {
  return `
pub struct ProductCatalogCommandRuntime {
    command_port: Arc<dyn ProductCatalogCommandPort>,
    media_asset_read_port: Option<Arc<dyn ProductMediaAssetReadPort>>,
    profile: ProductCatalogCommandProfile,
}
pub fn with_media_asset_read_port(mut self, media_asset_read_port: Arc<dyn ProductMediaAssetReadPort>) -> Self {
    self.command_port = Arc::new(ProductMediaValidatedCommandPort::new(
        self.command_port.clone(),
        media_asset_read_port.clone(),
    ));
    self.media_asset_read_port = Some(media_asset_read_port);
    self
}
pub const fn media_reference_policy(&self) -> ProductMediaReferencePolicy {
    if self.media_asset_read_port.is_some() {
        ProductMediaReferencePolicy::ValidatedByMediaOwner
    } else {
        ProductMediaReferencePolicy::OpaqueReferences
    }
}
`;
}

function hostAdapterSource() {
  return `
pub struct ProductMediaAssetProvider {
    media_asset_read_port: Arc<dyn MediaAssetReadPort>,
}
impl ProductMediaAssetValidationPort for ProductMediaAssetProvider {
    async fn ensure_asset_exists(&self, context: PortContext, media_id: Uuid) -> Result<(), PortError> {
        self.media_asset_read_port
            .get_asset(context, media_id)
            .await
            .map(|_| ())
    }
}
pub fn compose_product_media_asset_validation(
    runtime: ProductCatalogCommandRuntime,
    server: &ServerRuntimeContext,
    host: &HostRuntimeContext,
) -> ProductCatalogCommandRuntime {
    if runtime.media_asset_read_port().is_some() {
        return runtime;
    }
    let Some(provider) = resolve_media_asset_read_port(server, host) else {
        return runtime;
    };
    runtime.with_media_asset_read_port(Arc::new(ProductMediaAssetProvider::new(provider)))
}
`;
}

function hostCompositionSource({ includeComposition = true } = {}) {
  return `
fn attach(host: HostRuntimeContext, server: &ServerRuntimeContext) -> HostRuntimeContext {
    let runtime = rustok_product::ProductCatalogCommandRuntime::in_process(db, bus);
    ${includeComposition ? "let runtime = compose_product_catalog_media_asset_validation(runtime, server, &host);" : ""}
    server.shared_insert(runtime.clone());
    host.with_shared_value(runtime)
}
fn compose_product_catalog_media_asset_validation(runtime: ProductCatalogCommandRuntime, server: &ServerRuntimeContext, host: &HostRuntimeContext) -> ProductCatalogCommandRuntime {
    crate::services::product_media_asset_validation::compose_product_media_asset_validation(runtime, server, host)
}
`;
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-storefront-media-"));
  writeFixtureFile(
    root,
    "crates/modules/rustok-product/src/services/catalog/types.rs",
    ownerTypesSource(options)
  );
  writeFixtureFile(
    root,
    "crates/modules/rustok-product/src/services/catalog/queries.rs",
    ownerQueriesSource(options)
  );
  writeFixtureFile(root, "crates/modules/rustok-commerce/src/graphql/types.rs", commerceTypesSource());
  writeFixtureFile(
    root,
    "crates/modules/rustok-commerce/src/graphql/product_catalog.rs",
    commerceCatalogSource(options)
  );
  writeFixtureFile(root, "crates/modules/rustok-product/storefront/src/model.rs", leptosModelSource());
  writeFixtureFile(root, "crates/modules/rustok-product/storefront/src/core.rs", leptosCoreSource());
  writeFixtureFile(
    root,
    "crates/modules/rustok-product/storefront/src/ui/leptos.rs",
    leptosUiSource(options)
  );
  writeFixtureFile(
    root,
    "crates/modules/rustok-product/storefront/src/transport/graphql_adapter.rs",
    leptosGraphqlSource()
  );
  writeFixtureFile(
    root,
    "crates/modules/rustok-product/storefront/src/transport/native_server_adapter.rs",
    leptosNativeSource()
  );
  writeFixtureFile(
    root,
    "apps/next-frontend/packages/rustok-product/src/api/products.ts",
    nextApiSource()
  );
  writeFixtureFile(
    root,
    "apps/next-frontend/packages/rustok-product/src/api/types.ts",
    nextTypesSource()
  );
  writeFixtureFile(
    root,
    "apps/next-frontend/packages/rustok-product/src/components/product-card.tsx",
    nextCardSource(options)
  );
  writeFixtureFile(
    root,
    "apps/next-frontend/packages/rustok-product/src/components/product-detail-view.tsx",
    nextDetailSource()
  );
  writeFixtureFile(
    root,
    "apps/next-admin/packages/rustok-product/src/api/products.ts",
    ffaProductsSource(options)
  );
  writeFixtureFile(
    root,
    "apps/next-admin/packages/rustok-product/src/pages/product-editor-page.tsx",
    ffaEditorSource()
  );
  writeFixtureFile(
    root,
    "apps/next-admin/src/app/dashboard/product/actions.ts",
    ffaActionsSource(options)
  );
  writeFixtureFile(
    root,
    "crates/modules/rustok-product/src/media_asset_read_port.rs",
    productPortSource(options)
  );
  writeFixtureFile(
    root,
    "crates/modules/rustok-product/src/runtime.rs",
    productRuntimeSource()
  );
  writeFixtureFile(
    root,
    "apps/server/src/services/product_media_asset_validation.rs",
    hostAdapterSource()
  );
  writeFixtureFile(
    root,
    "apps/server/src/services/commerce_provider_runtime.rs",
    hostCompositionSource(options)
  );
  return root;
}

function runVerifier(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

function expectFailure(options, pattern) {
  const root = withFixture(options);
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected fixture to fail verification");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("storefront media/price verifier passes the canonical fixture", () => {
  const root = withFixture();
  try {
    const result = runVerifier(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /source-locked/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("verifier requires the owner list price snapshot", () => {
  expectFailure({ includePriceField: false }, /list item must expose price_from/);
});

test("verifier requires the batched owner media loader", () => {
  expectFailure({ includeMediaBatch: false }, /published list must batch-load catalog-card media/);
});

test("verifier requires the GraphQL catalog currency filter wiring", () => {
  expectFailure({ includeCurrency: false }, /filter currency must reach the owner query/);
});

test("verifier requires the Leptos catalog card image", () => {
  expectFailure({ includeCardImage: false }, /catalog card must render the product image/);
});

test("verifier requires the Next catalog card owner image", () => {
  expectFailure({ includeImage: false }, /catalog card must render the owner-resolved image/);
});

test("verifier requires the FFA attribute idempotency key", () => {
  expectFailure(
    { includeIdempotency: false },
    /save-attribute-values mutation must require \$idempotencyKey/
  );
});

test("verifier requires the FFA partial attribute failure surface", () => {
  expectFailure(
    { includePartialFailure: false },
    /partial attribute failure must be reported, not swallowed/
  );
});

test("verifier requires media asset validation before an image write", () => {
  expectFailure(
    { includeValidation: false },
    /add_product_image must validate the new media reference first/
  );
});

test("verifier requires the host to compose media asset validation", () => {
  expectFailure(
    { includeComposition: false },
    /Product command composition must attach media asset validation/
  );
});
