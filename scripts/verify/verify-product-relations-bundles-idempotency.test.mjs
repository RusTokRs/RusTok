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
  "crates/modules/rustok-product-relations/src/dto.rs",
  "crates/modules/rustok-product-relations/src/services/mod.rs",
  "crates/modules/rustok-product-relations/src/lib.rs",
  "crates/modules/rustok-product-relations/Cargo.toml",
  "crates/modules/rustok-product-relations/admin/src/transport/native_server_adapter.rs",
  "crates/modules/rustok-product-relations/src/migrations/mod.rs",
  "crates/modules/rustok-product-relations/src/migrations/m20261017_000005_remove_redundant_product_tenant_key.rs",
  "crates/modules/rustok-product-bundles/src/services/receipts.rs",
  "crates/modules/rustok-product-bundles/src/services/bundle_service.rs",
  "crates/modules/rustok-product-bundles/src/dto.rs",
  "crates/modules/rustok-product-bundles/src/migrations/mod.rs",
  "crates/modules/rustok-product-bundles/src/migrations/m20261017_000002_normalize_bundle_translation_locales.rs",
  "crates/modules/rustok-product-bundles/src/migrations/m20261017_000004_enforce_bundle_tenant_integrity.rs",
  "crates/modules/rustok-product-bundles/src/migrations/m20260916_000001_create_bundles.rs",
  "crates/modules/rustok-product/src/migrations/m20261017_000036_add_sqlite_tenant_identity_keys.rs",
  "crates/modules/rustok-product/src/services/write_transaction.rs",
  "crates/modules/rustok-product/src/services/catalog_schema_service/attributes.rs",
  "crates/modules/rustok-product/src/translation_target.rs",
  "crates/modules/rustok-product/src/variant_translation_target.rs",
  "crates/modules/rustok-product/src/image_translation_target.rs",
  "crates/modules/rustok-product-bundles/src/services/mod.rs",
  "crates/modules/rustok-product-bundles/src/lib.rs",
  "crates/modules/rustok-product-bundles/Cargo.toml",
  "crates/modules/rustok-product-bundles/admin/src/transport/native_server_adapter.rs",
  "crates/modules/rustok-outbox/src/idempotency.rs",
  "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs",
  "crates/modules/rustok-commerce/src/graphql/types.rs",
  "crates/modules/rustok-commerce/src/graphql/product_catalog.rs",
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
  assert.match(result.output, /complete tenant-scoped owner receipts/);
});

test("a receipt that is never completed fails the contract", () => {
  const relative = "crates/modules/rustok-product-relations/src/services/receipts.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "complete_relation_operation(txn, context, lease, &relation).await?;",
      "",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /create_relation_idempotent receipt command: missing complete_relation_operation\(txn, context, lease, &relation\)\.await\?;/,
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

test("the Product receipt rollback boundary must remain documented", () => {
  const relative = "crates/modules/rustok-product/docs/write-boundary.md";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "receipt-result encoding, a missing result, or completion failure awaits transaction rollback",
      "receipt finalization behavior is unspecified",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /write boundary doc: missing receipt-result encoding, a missing result, or completion failure awaits transaction rollback/,
  );
});

test("a terminal relation failure must await rollback before finalizing its receipt", () => {
  const relative = "crates/modules/rustok-product-relations/src/services/receipts.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "Err(error) => {\n                rollback_relation_operation(txn, context, lease).await?;",
      "Err(error) => {\n                drop(txn);",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /create_relation_idempotent receipt command: missing rollback_relation_operation\(txn, context, lease\)\.await\?;/,
  );
});

test("a receipt completion error must await rollback before returning", () => {
  const relative = "crates/modules/rustok-product-relations/src/services/receipts.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "if let Err(receipt_error) = idempotency::complete(&txn, lease, response).await {\n        rollback_relation_operation(txn, context, lease).await?;",
      "if let Err(receipt_error) = idempotency::complete(&txn, lease, response).await {\n        drop(txn);",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /relations successful receipt completion: missing rollback_relation_operation\(txn, context, lease\)\.await\?;/,
  );
});

test("a Product owner receipt completion error must await rollback before returning", () => {
  const relative = "crates/modules/rustok-product/src/services/write_transaction.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "        return rollback_product_receipt_failure(\n            transaction,\n            receipt.lease,\n            CommerceError::Database(DbErr::Custom(format!(\n                \"product owner receipt completion failed: {}\",\n                error.code\n            ))),\n        )\n        .await;",
      "        drop(transaction);\n        return Err(CommerceError::Database(DbErr::Custom(format!(\n            \"product owner receipt completion failed: {}\",\n            error.code\n        ))));",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /product owner receipt completion: forbidden drop\(transaction\);/,
  );
});

test("a Product receipt result encoding failure must remain deferred to commit", () => {
  const relative = "crates/modules/rustok-product/src/services/write_transaction.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "*slot = Some(response_json);",
      "*slot = None;",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /product owner receipt result recording: missing \*slot = Some\(response_json\);/,
  );
});

test("a receipt-scoped early Product owner error must hand off its transaction for an awaited rollback", () => {
  const relative = "crates/modules/rustok-product/src/services/write_transaction.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "receipt.defer_rollback(transaction);",
      "drop(transaction);",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /product owner early-error rollback: missing receipt\.defer_rollback\(transaction\);/,
  );
});

test("a receipt wrapper must preserve custom Product owner errors while awaiting rollback", () => {
  const relative = "crates/modules/rustok-product/src/services/write_transaction.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(relative, "    E: From<CommerceError>,\n", ""),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /product owner receipt scope rollback fence: missing E: From<CommerceError>/,
  );
});

test("a retryable Product failure must not become a terminal receipt", () => {
  const relative = "crates/modules/rustok-product/src/services/write_transaction.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "    if error.retryable {\n        return Ok(());\n    }\n\n    idempotency::fail(database, lease, error).await",
      "    idempotency::fail(database, lease, error).await",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /product retryable receipt finalization: missing if error\.retryable \{/,
  );
});

test("a Product translation target cannot bypass retryable receipt handling", () => {
  const relative = "crates/modules/rustok-product/src/translation_target.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "fail_product_operation_receipt(self.service.database(), lease, error).await",
      "idempotency::fail(self.service.database(), lease, error).await",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /Product translation-target receipt finalizer: forbidden idempotency::fail\(/,
  );
});

test("a Product owner receipt completion cannot return without rollback", () => {
  const relative = "crates/modules/rustok-product/src/services/write_transaction.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "        return rollback_product_receipt_failure(\n            transaction,\n            receipt.lease,\n            CommerceError::Database(DbErr::Custom(format!(\n                \"product owner receipt completion failed: {}\",\n                error.code\n            ))),\n        )\n        .await;",
      "        return Err(CommerceError::Database(DbErr::Custom(format!(\n            \"product owner receipt completion failed: {}\",\n            error.code\n        ))));",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /product owner receipt completion failure: missing return rollback_product_receipt_failure\(/,
  );
});

test("a missing Product receipt result cannot return without rollback", () => {
  const relative = "crates/modules/rustok-product/src/services/write_transaction.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "return rollback_product_receipt_failure(transaction, receipt.lease, error).await;",
      "return Err(error);",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /product owner receipt result failure: missing return rollback_product_receipt_failure\(transaction, receipt\.lease, error\)\.await;/,
  );
});

test("a bundle receipt completion error must await rollback before returning", () => {
  const relative = "crates/modules/rustok-product-bundles/src/services/receipts.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "if let Err(receipt_error) = idempotency::complete(&txn, lease, response).await {\n        rollback_bundle_operation(txn, context, lease).await?;",
      "if let Err(receipt_error) = idempotency::complete(&txn, lease, response).await {\n        drop(txn);",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /bundles successful receipt completion: missing rollback_bundle_operation\(txn, context, lease\)\.await\?;/,
  );
});

test("failure finalization cannot ignore a lost receipt lease", () => {
  const relative = "crates/modules/rustok-outbox/src/idempotency.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "owner operation receipt was not processing during failure finalization",
      "failure finalization outcome is ignored",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /shared failure receipt finalization: missing owner operation receipt was not processing during failure finalization/,
  );
});


test("Bundle GraphQL must preserve an explicit null owner clear", () => {
  const relative = "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "bundle_product_id: bundle_product_id_update(input.bundle_product_id)",
      "bundle_product_id: input.bundle_product_id.map(Some)",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /update_bundle GraphQL mutation: missing bundle_product_id: bundle_product_id_update\(input\.bundle_product_id\)/,
  );
});

test("Bundle native commands cannot expose raw database errors", () => {
  const relative =
    "crates/modules/rustok-product-bundles/admin/src/transport/native_server_adapter.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "BundleCommandError::Domain(error) => bundle_error_copy(error)",
      "BundleCommandError::Domain(error) => error.to_string()",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /bundles native adapter: forbidden BundleCommandError::Domain\(error\) => error\.to_string\(\)/,
  );
});


test("a GraphQL adapter cannot coerce an unknown relation type to RELATED", () => {
  const relative =
    "crates/modules/rustok-product-relations/admin/src/transport/graphql_adapter.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "RelationType::Custom(_) => {",
      '_ => Ok("RELATED"),',
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /relations remote graphql adapter type mapping: missing RelationType::Custom\(_\) => \{/,
  );
});

test("GraphQL relation enum ingress keeps its total domain conversion", () => {
  const relative = "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "relation_type: input.relation_type.into(),",
      "relation_type: input.relation_type.try_into().unwrap(),",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /add_product_relation GraphQL relation type ingress: missing relation_type: input\.relation_type\.into\(\),/,
  );
});

test("product relation GraphQL reads require product read permission", () => {
  const relative = "crates/modules/rustok-commerce/src/graphql/product_catalog.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "        require_commerce_permission(\n            ctx,\n            &[Permission::PRODUCTS_READ],\n            \"Permission denied: products:read required\",\n        )?;\n",
      "",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /product_relations GraphQL query authorization: missing require_commerce_permission\(/,
  );
});


test("bundle GraphQL reads cannot lose their product-read authorization", () => {
  const relative = "crates/modules/rustok-commerce/src/graphql/product_catalog.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "        require_product_bundle_read(ctx)?;\n",
      "",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /bundle GraphQL query authorization: missing require_product_bundle_read\(ctx\)\?;/,
  );
});


test("a Next bundle add must preserve omitted position for append allocation", () => {
  const relative = "apps/next-admin/packages/rustok-product/src/api/bundles.ts";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "...(input.position === undefined ? {} : { position: input.position })",
      "position: input.position ?? 0",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /next bundles client: forbidden position: input\.position \?\? 0/);
});

test("Bundle receipt serialization cannot collapse owner keep and clear", () => {
  const relative = "crates/modules/rustok-product-bundles/src/dto.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      `#[serde(
        default,
        deserialize_with = "deserialize_optional_optional",
        skip_serializing_if = "Option::is_none"
    )]
    pub bundle_product_id: Option<Option<Uuid>>`,
      "pub bundle_product_id: Option<Option<Uuid>>",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /bundles update patch DTO: missing #\[serde\(/);
});

test("Bundle GraphQL description patches cannot silently disappear", () => {
  const relative = "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "fn bundle_translation_updates(",
      "fn removed_bundle_translation_updates(",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /Bundle GraphQL update patch mapping: missing fn bundle_translation_updates\(/);
});

test("relation delete cannot silently accept a zero-row mutation", () => {
  const relative = "crates/modules/rustok-product-relations/src/services/relation_service.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "if delete.rows_affected != 1",
      "if false",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /relation mutation lock and delete result: missing if delete\.rows_affected != 1/);
});

test("Bundle tenant integrity cannot be left only in a rewritten historical migration", () => {
  const relative =
    "crates/modules/rustok-product-bundles/src/migrations/m20261017_000004_enforce_bundle_tenant_integrity.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "CREATE TABLE product_bundle_items_tenant_integrity",
      "CREATE TABLE product_bundle_items_without_tenant_integrity",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /bundles tenant-integrity upgrade migration: missing CREATE TABLE product_bundle_items_tenant_integrity/,
  );
});

test("Bundle items keep Variant-to-selected-Product foreign-key integrity", () => {
  const relative =
    "crates/modules/rustok-product-bundles/src/migrations/m20261017_000004_enforce_bundle_tenant_integrity.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "variant.product_id IS DISTINCT FROM item.product_id",
      "variant.product_id = item.product_id",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /bundles Variant-to-Product integrity migration: missing variant\.product_id IS DISTINCT FROM item\.product_id/,
  );
});

test("Bundle Variant integrity retains its Product-owned parent key", () => {
  const relative =
    "crates/modules/rustok-product/src/migrations/m20261017_000036_add_sqlite_tenant_identity_keys.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "UNIQUE (tenant_id, product_id, id)",
      "UNIQUE (tenant_id, id)",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /Product Variant composite parent key: missing UNIQUE \(tenant_id, product_id, id\)/,
  );
});

test("Relation tenant-key cleanup cannot drop a referenced key without rebuilding both FKs", () => {
  const relative =
    "crates/modules/rustok-product-relations/src/migrations/m20261017_000005_remove_redundant_product_tenant_key.rs";
  const fixtureRoot = writeFixture({
    [relative]: replaceRequired(
      relative,
      "DROP CONSTRAINT IF EXISTS fk_product_relations_related_product",
      "-- related-product foreign key not rebuilt",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(
    result.output,
    /relations tenant-key cleanup migration: missing DROP CONSTRAINT IF EXISTS fk_product_relations_related_product/,
  );
});
