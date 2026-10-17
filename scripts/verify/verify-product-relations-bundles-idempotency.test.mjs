import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, "../..");
const verifier = path.join(here, "verify-product-relations-bundles-idempotency.mjs");

// Keep the fixture synchronized with the verifier's declared input surface. Each test starts with
// the real checked-in contract and mutates precisely one invariant, rather than maintaining a
// second, stale miniature of every Rust and GraphQL source file.
const verifierInputs = [
  "crates/modules/rustok-product-relations/src/services/receipts.rs",
  "crates/modules/rustok-product-relations/src/services/relation_service.rs",
  "crates/modules/rustok-product-relations/src/services/mod.rs",
  "crates/modules/rustok-product-relations/src/lib.rs",
  "crates/modules/rustok-product-relations/Cargo.toml",
  "crates/modules/rustok-product-relations/admin/src/transport/native_server_adapter.rs",
  "crates/modules/rustok-product-bundles/src/services/receipts.rs",
  "crates/modules/rustok-product-bundles/src/services/bundle_service.rs",
  "crates/modules/rustok-product-bundles/src/services/mod.rs",
  "crates/modules/rustok-product-bundles/src/lib.rs",
  "crates/modules/rustok-product-bundles/Cargo.toml",
  "crates/modules/rustok-product-bundles/admin/src/transport/native_server_adapter.rs",
  "crates/modules/rustok-outbox/src/idempotency.rs",
  "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs",
  "crates/modules/rustok-product-relations/admin/src/transport/graphql_adapter.rs",
  "crates/modules/rustok-product-bundles/admin/src/transport/graphql_adapter.rs",
  "apps/next-admin/packages/rustok-product/src/api/relations.ts",
  "apps/next-admin/packages/rustok-product/src/api/bundles.ts",
  "crates/modules/rustok-product/docs/write-boundary.md",
  "docs/modules/registry.md",
  "docs/audits/product-module-engineering-audit-2026-10-07.md",
];

const source = (relative) => fs.readFileSync(path.join(repoRoot, relative), "utf8");

const writeFixture = (overrides = {}) => {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "verify-catalog-idem-"));
  for (const relative of verifierInputs) {
    const target = path.join(fixtureRoot, relative);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, source(relative));
  }
  for (const [relative, content] of Object.entries(overrides)) {
    const target = path.join(fixtureRoot, relative);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, content);
  }
  return fixtureRoot;
};

const replaceRequired = (relative, from, to) => {
  const original = source(relative);
  assert.ok(original.includes(from), `fixture mutation anchor is missing: ${from}`);
  return original.replace(from, to);
};

const runVerifier = (fixtureRoot) => {
  try {
    const stdout = execFileSync(process.execPath, [verifier], {
      cwd: repoRoot,
      env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: fixtureRoot },
      encoding: "utf8",
    });
    return { status: 0, output: stdout };
  } catch (error) {
    return { status: error.status ?? 1, output: `${error.stdout ?? ""}${error.stderr ?? ""}` };
  }
};

test("canonical receipt-bound relations and bundles pass", () => {
  const fixtureRoot = writeFixture();
  const result = runVerifier(fixtureRoot);
  assert.equal(result.status, 0, result.output);
  assert.match(result.output, /write through tenant-scoped owner receipts/);
});

test("a receipt that is never completed fails the contract", () => {
  const relative = "crates/modules/rustok-product-relations/src/services/receipts.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(relative, "idempotency::complete(&txn, lease, &relation).await?;", ""),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /create_relation_idempotent receipt command: missing idempotency::complete\(&txn, lease, &relation\)\.await\?;/,
  );
});

test("a Commerce mutation cannot return to a plain relation service call", () => {
  const relative = "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(relative, ".create_relation_idempotent(", ".create_relation("),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /catalog write ingress: forbidden \.create_relation\(/);
});

test("an admin adapter that discards the caller key fails the contract", () => {
  const relative =
    "crates/modules/rustok-product-bundles/admin/src/transport/native_server_adapter.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "        let service = BundleService::new(runtime.db_clone());",
      "        let _ = idempotency_key;\n        let service = BundleService::new(runtime.db_clone());",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /bundles native adapter: forbidden let _ = idempotency_key;/);
});

test("an optional GraphQL idempotency key fails the contract", () => {
  const relative = "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "        idempotency_key: String,\n        input: AddProductRelationInput,",
      "        idempotency_key: Option<String>,\n        input: AddProductRelationInput,",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /add_product_relation GraphQL mutation: missing idempotency_key: String/);
});

test("a parallel receipt ledger fails the contract", () => {
  const relative = "crates/modules/rustok-product-relations/src/services/receipts.rs";
  const fixtureRoot = writeFixture({
    [relative]: `${source(relative)}
#[derive(DeriveEntityModel)]
#[sea_orm(table_name = "owner_operation_receipts")]
pub struct Model {}
`,
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /relations receipts: forbidden DeriveEntityModel/);
});

test("a Next admin client that stops sending its key fails the contract", () => {
  const relative = "apps/next-admin/packages/rustok-product/src/api/bundles.ts";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "removeBundleItem(idempotencyKey: $idempotencyKey, bundleId: $bundleId, itemId: $itemId)",
      "removeBundleItem(bundleId: $bundleId, itemId: $itemId)",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /next bundles client: missing removeBundleItem\(idempotencyKey: \$idempotencyKey, bundleId: \$bundleId, itemId: \$itemId\)/,
  );
});

test("Commerce must use the owner-exported caller-key scope", () => {
  const relative = "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "scoped_relation_caller_key(tenant_id, user_id, operation, None, caller_key)",
      "scoped_catalog_operation_key(tenant_id, user_id, operation, None, caller_key)",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /add_product_relation GraphQL mutation: missing scoped_relation_caller_key/);
});

test("an undocumented write boundary fails the contract", () => {
  const fixtureRoot = writeFixture({
    "crates/modules/rustok-product/docs/write-boundary.md": "# Product write boundary\n",
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /write boundary doc: missing/);
});
