#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve(
  "scripts/verify/verify-product-http-checkout-catalog-runtime.mjs",
);

function write(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-http-checkout-"));
  write(
    root,
    "crates/rustok-commerce/src/controllers/mod.rs",
    options.missingRuntime
      ? "struct CommerceHttpRuntime {}"
      : `
        struct CommerceHttpRuntime { product_catalog_read_runtime: rustok_product::ProductCatalogReadRuntime }
        fn from_host() { shared_get::<rustok_product::ProductCatalogReadRuntime>(); "Commerce HTTP routes require ProductCatalogReadRuntime in HostRuntimeContext"; }
        fn product_catalog_read_port(&self) { self.product_catalog_read_runtime.read_port(); }
      `,
  );
  const handlerCall = options.embeddedHandler
    ? "complete_storefront_checkout_input("
    : "complete_storefront_checkout_input_with_product_port(runtime.product_catalog_read_port(),";
  write(
    root,
    "crates/rustok-commerce/src/controllers/store/checkout.rs",
    `pub async fn complete_cart_checkout() { ${handlerCall} }`,
  );
  write(
    root,
    "crates/rustok-commerce/src/storefront_staged_checkout_runtime.rs",
    options.composedCatalog
      ? "pub async fn complete_storefront_checkout_input_with_product_port() { CatalogService::new; }"
      : "pub async fn complete_storefront_checkout_input_with_product_port() { product_catalog_read_port; }",
  );
  const complete = options.registryPending
    ? ["ai-product", "marketplace-listing", "order-storefront-native"]
    : [
        "ai-product",
        "marketplace-listing",
        "order-storefront-native",
        "commerce-checkout-http",
      ];
  const pending = options.registryPending
    ? ["commerce-checkout-http", "commerce-checkout-graphql"]
    : ["commerce-checkout-graphql"];
  write(
    root,
    "crates/rustok-product/contracts/product-fba-registry.json",
    JSON.stringify({
      runtime_composition: {
        source_complete_consumers: complete,
        pending_consumers: pending,
        status: "source_complete_consumer_cutover_partial",
      },
    }),
  );
  write(
    root,
    "crates/rustok-product/docs/implementation-plan.md",
    options.omitPlan
      ? "Product plan"
      : "Commerce HTTP checkout Commerce GraphQL checkout verify-product-http-checkout-catalog-runtime.mjs remains open only for that surface",
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
    assert.notEqual(result.status, 0, "expected HTTP checkout mutation to fail");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("HTTP checkout runtime guard accepts canonical fixture", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("HTTP checkout guard rejects missing host runtime", () => {
  reject({ missingRuntime: true }, /Commerce HTTP runtime/);
});

test("HTTP checkout guard rejects embedded compatibility call", () => {
  reject({ embeddedHandler: true }, /embedded compatibility wrapper/);
});

test("HTTP checkout guard rejects Product construction in composed body", () => {
  reject({ composedCatalog: true }, /must not construct CatalogService/);
});

test("HTTP checkout guard rejects stale registry pending state", () => {
  reject({ registryPending: true }, /commerce-checkout-http/);
});

test("HTTP checkout guard rejects missing plan handoff", () => {
  reject({ omitPlan: true }, /Product implementation plan/);
});
