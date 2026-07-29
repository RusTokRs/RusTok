#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve(
  "scripts/verify/verify-product-graphql-checkout-catalog-runtime.mjs",
);

function write(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-graphql-checkout-"));
  const runtimeSource = options.missingRuntime
    ? "pub struct CommerceGraphqlRuntimeData;"
    : `
      use rustok_product::ProductCatalogReadRuntime;
      tokio::task_local! { static CURRENT_COMMERCE_PRODUCT_CATALOG_READ_RUNTIME: ProductCatalogReadRuntime; }
      struct CommerceGraphqlRuntimeData { product_catalog_read_runtime: ProductCatalogReadRuntime }
      impl CommerceGraphqlRuntimeData { pub fn product_catalog_read_runtime(&self) { } }
      fn scope() { runtime_data.product_catalog_read_runtime(); CURRENT_COMMERCE_PRODUCT_CATALOG_READ_RUNTIME; }
      fn attach_schema_data() { shared_get::<ProductCatalogReadRuntime>(); "commerce GraphQL requires ProductCatalogReadRuntime in host composition"; }
      pub(crate) fn product_catalog_read_runtime_for_current_graphql_scope() { try_with(Clone::clone); ProductCatalogReadRuntime::in_process(db, event_bus); }
    `;
  write(root, "crates/rustok-commerce/src/graphql_runtime.rs", runtimeSource);
  const wrapperCatalog = options.wrapperCatalog ? "CatalogService::new;" : "";
  const wrapperScope = options.missingScope
    ? ""
    : "product_catalog_read_runtime_for_current_graphql_scope; .read_port();";
  write(
    root,
    "crates/rustok-commerce/src/storefront_staged_checkout_runtime.rs",
    `pub async fn complete_storefront_checkout_input() { ${wrapperScope} complete_storefront_checkout_input_with_product_port; ${wrapperCatalog} }`,
  );
  write(
    root,
    "crates/rustok-commerce/src/graphql/mutations/checkout.rs",
    options.mutationCatalog
      ? "async fn complete_storefront_checkout() { CatalogService::new; }"
      : "async fn complete_storefront_checkout() { complete_storefront_checkout_input; }",
  );
  const complete = options.registryPending
    ? ["commerce-checkout-http"]
    : [
        "ai-product",
        "marketplace-listing",
        "order-storefront-native",
        "commerce-checkout-http",
        "commerce-checkout-graphql",
      ];
  write(
    root,
    "crates/rustok-product/contracts/product-fba-registry.json",
    JSON.stringify({
      evidence: {
        graphql_checkout_runtime_verifier:
          "scripts/verify/verify-product-graphql-checkout-catalog-runtime.mjs",
      },
      runtime_composition: {
        source_complete_consumers: complete,
        pending_consumers: options.registryPending ? ["commerce-checkout-graphql"] : [],
        status: options.registryPending
          ? "source_complete_consumer_cutover_partial"
          : "source_complete_consumer_cutover_complete",
      },
    }),
  );
  write(
    root,
    "crates/rustok-product/docs/implementation-plan.md",
    options.omitPlan
      ? "Product plan"
      : "mounted Commerce GraphQL checkout resolver-scoped task-local checkout consumer source cutover is complete verify-product-graphql-checkout-catalog-runtime.mjs Concrete external transport execution remains open",
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
    assert.notEqual(result.status, 0, "expected GraphQL checkout mutation to fail");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("GraphQL checkout runtime guard accepts canonical fixture", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("GraphQL checkout guard rejects missing schema runtime", () => {
  reject({ missingRuntime: true }, /Commerce GraphQL runtime/);
});

test("GraphQL checkout guard rejects missing resolver scope", () => {
  reject({ missingScope: true }, /compatibility wrapper/);
});

test("GraphQL checkout guard rejects Product construction in wrapper", () => {
  reject({ wrapperCatalog: true }, /must not construct CatalogService/);
});

test("GraphQL checkout guard rejects Product construction in mutation", () => {
  reject({ mutationCatalog: true }, /mutation must not construct CatalogService/);
});

test("GraphQL checkout guard rejects stale registry state", () => {
  reject({ registryPending: true }, /source-complete|pending consumers/);
});

test("GraphQL checkout guard rejects missing plan handoff", () => {
  reject({ omitPlan: true }, /Product implementation plan/);
});
