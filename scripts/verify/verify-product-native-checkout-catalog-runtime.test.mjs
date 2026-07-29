#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve(
  "scripts/verify/verify-product-native-checkout-catalog-runtime.mjs",
);

function write(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-native-checkout-"));
  const composedCatalog = options.composedCatalog ? "CatalogService::new" : "";
  write(
    root,
    "crates/rustok-commerce/src/storefront_staged_checkout_runtime.rs",
    `
    pub async fn complete_storefront_checkout_with_product_port(product_catalog_read_port: Arc<dyn rustok_product::ProductCatalogReadPort>) { complete_storefront_checkout_input_with_product_port(); }
    pub async fn complete_storefront_checkout_input() { CatalogService::new; }
    pub async fn complete_storefront_checkout_input_with_product_port(product_catalog_read_port: Arc<dyn rustok_product::ProductCatalogReadPort>) { CheckoutPlanBuilder::new; product_catalog_read_port; ${composedCatalog} }
    `,
  );
  write(
    root,
    "crates/rustok-order/storefront/src/transport/native_server_adapter/server_functions.rs",
    options.nativeDirect
      ? "CatalogService::new"
      : `shared_get::<ProductCatalogReadRuntime>() .read_port() complete_storefront_checkout_with_product_port dependency = "ProductCatalogReadRuntime"`,
  );
  write(
    root,
    "crates/rustok-order/storefront/Cargo.toml",
    options.omitProductDependency
      ? `[features]\nhydrate = ["leptos/hydrate"]\nssr = ["leptos/ssr"]\n[dependencies]`
      : `[features]\nhydrate = ["leptos/hydrate"]\nssr = ["leptos/ssr", "dep:rustok-product"]\n[dependencies]\nrustok-product = { workspace = true, optional = true }`,
  );
  const complete = options.registryPending
    ? ["ai-product", "marketplace-listing"]
    : ["ai-product", "marketplace-listing", "order-storefront-native"];
  const pending = options.registryPending
    ? ["commerce-checkout-http", "commerce-checkout-graphql", "order-storefront-native"]
    : ["commerce-checkout-http", "commerce-checkout-graphql"];
  write(
    root,
    "crates/rustok-product/contracts/product-fba-registry.json",
    JSON.stringify({
      runtime_composition: {
        source_complete_consumers: complete,
        pending_consumers: pending,
      },
    }),
  );
  write(
    root,
    "crates/rustok-product/docs/implementation-plan.md",
    options.omitPlan
      ? "Product plan"
      : "Order storefront native checkout complete_storefront_checkout_with_product_port Commerce HTTP and GraphQL checkout verify-product-native-checkout-catalog-runtime.mjs",
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
    assert.notEqual(result.status, 0, "expected native checkout mutation to fail");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("native checkout runtime guard accepts canonical fixture", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("native checkout guard rejects Product construction in composed body", () => {
  reject({ composedCatalog: true }, /must not construct CatalogService/);
});

test("native checkout guard rejects direct native Product construction", () => {
  reject({ nativeDirect: true }, /Order native checkout/);
});

test("native checkout guard rejects missing Product SSR dependency", () => {
  reject({ omitProductDependency: true }, /Order storefront SSR dependency/);
});

test("native checkout guard rejects stale registry pending state", () => {
  reject({ registryPending: true }, /order-storefront-native/);
});

test("native checkout guard rejects missing plan handoff", () => {
  reject({ omitPlan: true }, /Product implementation plan/);
});
