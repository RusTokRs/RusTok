#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve(
  "scripts/verify/verify-product-catalog-read-runtime-composition.mjs",
);

function write(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-read-runtime-"));
  write(
    root,
    "crates/modules/rustok-product/src/runtime.rs",
    options.omitExternal
      ? `pub enum ProductCatalogReadProfile {
    EmbeddedNative,
}
pub struct ProductCatalogReadRuntime {}
impl ProductCatalogReadRuntime {
    pub fn in_process(catalog: Arc<CatalogService>) -> Self {
        Self::default()
            .with_storefront_http_read_port(catalog.clone())
            .with_storefront_tag_read_port(catalog)
    }
    pub fn read_port(&self) -> Arc<dyn ProductCatalogReadPort> {}
    pub const fn profile(&self) -> ProductCatalogReadProfile {}
}
`
      : `pub enum ProductCatalogReadProfile {
    EmbeddedNative,
    External,
}
pub struct ProductCatalogReadRuntime {
    storefront_http_read_port: Option<Arc<dyn ProductStorefrontHttpReadPort>>,
    storefront_tag_read_port: Option<Arc<dyn ProductStorefrontTagReadPort>>,
}
impl ProductCatalogReadRuntime {
    pub fn in_process(catalog: Arc<CatalogService>) -> Self {
        Self::default()
            .with_storefront_http_read_port(catalog.clone())
            .with_storefront_tag_read_port(catalog)
    }
    pub fn external(read_port: Arc<dyn ProductCatalogReadPort>) -> Self {}
    pub fn with_storefront_http_read_port(mut self, port: Arc<dyn ProductStorefrontHttpReadPort>) -> Self {}
    pub fn storefront_http_read_port(&self) -> Option<Arc<dyn ProductStorefrontHttpReadPort>> {}
    pub fn with_storefront_tag_read_port(mut self, port: Arc<dyn ProductStorefrontTagReadPort>) -> Self {}
    pub fn storefront_tag_read_port(&self) -> Option<Arc<dyn ProductStorefrontTagReadPort>> {}
    pub fn read_port(&self) -> Arc<dyn ProductCatalogReadPort> {}
    pub const fn profile(&self) -> ProductCatalogReadProfile {}
}
`,
  );
  write(
    root,
    "crates/modules/rustok-product/src/lib.rs",
    `mod runtime;
mod storefront_http_read_port;
mod storefront_tag_read_port;
pub use runtime::{ProductCatalogReadProfile, ProductCatalogReadRuntime};
pub use storefront_http_read_port::{
    LegacyStorefrontHttpProductsRequest, ProductStorefrontHttpReadPort,
};
pub use storefront_tag_read_port::{
    ProductStorefrontTagHydrationRequest, ProductStorefrontTagReadPort,
};
`,
  );
  write(
    root,
    "crates/modules/rustok-product/src/storefront_http_read_port.rs",
    `pub trait ProductStorefrontHttpReadPort {}
impl ProductStorefrontHttpReadPort for CatalogService {}
const MAX_LEGACY_STOREFRONT_HTTP_PRODUCTS_PER_PAGE: u64 = 100;
fn load(context: &PortContext, products: &[Product]) {
    context.require_policy(PortCallPolicy::read())?;
    rustok_inventory::is_metadata_visible_for_public_channel(
        context,
        metadata,
        channel,
    );
    self.load_product_tag_map(tenant_id, &products, locale, Some(fallback_locale));
}
`,
  );
  write(
    root,
    "crates/modules/rustok-product/src/storefront_tag_read_port.rs",
    `pub trait ProductStorefrontTagReadPort {}
impl ProductStorefrontTagReadPort for CatalogService {}
const MAX_STOREFRONT_TAG_HYDRATION_PRODUCTS: usize = 48;
fn hydrate(&self) {
    self.load_product_tag_map(tenant_id, &products, locale, None);
}
`,
  );
  const directMarketplace = options.directMarketplace
    ? "let product_reader: Arc<dyn rustok_product::ProductCatalogReadPort> = Arc::new("
    : "";
  const directAi = options.directAi
    ? "rustok_product::CatalogService::new(server.db_clone(), event_bus)"
    : "";
  write(
    root,
    "apps/server/src/services/commerce_provider_runtime.rs",
    `host.shared_get::<rustok_product::ProductCatalogReadRuntime>()
server.shared_get::<rustok_product::ProductCatalogReadRuntime>()
rustok_product::ProductCatalogReadRuntime::in_process
ProductCatalogReadRuntime must be initialized before marketplace listing
preserves_host_selected_external_product_catalog_runtime
ProductCatalogReadProfile::External
host.with_shared_value(rustok_ai::SharedAiProductCatalogReadPort(
    runtime.read_port(),
))
${directMarketplace} ${directAi}`,
  );
  write(
    root,
    "crates/modules/rustok-product/contracts/product-fba-registry.json",
    options.omitRegistry
      ? "{}"
      : JSON.stringify({
          runtime_composition: {
            runtime: "ProductCatalogReadRuntime",
            profiles: ["embedded_native", "external"],
            source_complete_consumers: [
              "ai-product",
              "marketplace-listing",
              "order-storefront-native",
              "commerce-checkout-http",
              "commerce-checkout-graphql",
            ],
            pending_consumers: [],
            status: "source_complete_consumer_cutover_complete",
          },
        }, null, 2),
  );
  write(
    root,
    "crates/modules/rustok-product/docs/implementation-plan.md",
    options.omitPlan
      ? "Product plan"
      : `ProductCatalogReadRuntime AI, and the checkout consumer
source cutover is complete. Concrete external transport execution remains open:
verify-product-catalog-read-runtime-composition.mjs
`,
  );
  return root;
}

function run(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

function reject(options, pattern) {
  const root = fixture(options);
  try {
    const result = run(root);
    assert.notEqual(result.status, 0, "expected runtime-composition mutation to fail");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("runtime composition guard accepts canonical source fixture", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("runtime composition guard rejects missing external profile", () => {
  reject({ omitExternal: true }, /Product owner runtime/);
});

test("runtime composition guard rejects parallel Marketplace provider", () => {
  reject({ directMarketplace: true }, /parallel Marketplace Product provider/);
});

test("runtime composition guard rejects parallel AI provider", () => {
  reject({ directAi: true }, /parallel AI Product provider/);
});

test("runtime composition guard rejects missing registry evidence", () => {
  reject({ omitRegistry: true }, /Product FBA registry/);
});

test("runtime composition guard rejects missing plan handoff", () => {
  reject({ omitPlan: true }, /Product implementation plan/);
});
