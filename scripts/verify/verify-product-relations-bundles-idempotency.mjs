#!/usr/bin/env node
//
// PROD-IDEM-001 guard: Product owner mutations, Product relations, and Product bundles are
// write-like owner commands, so every external entry point must carry a caller-owned idempotency
// key and complete a durable owner receipt inside the same transaction as the write.
//
// The guard pins the Product transaction wrapper and the receipt protocol of both owner crates,
// the transaction cores the receipt-bound commands share with the plain port methods, the required
// GraphQL/native arguments, and the rejection of any parallel receipt ledger.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");
const failures = [];
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const slice = (source, start, end) => {
  const startIndex = source.indexOf(start);
  if (startIndex < 0) {
    failures.push(`missing slice anchor ${start}`);
    return "";
  }
  const endIndex = source.indexOf(end, startIndex + 1);
  return source.slice(startIndex, endIndex < 0 ? source.length : endIndex);
};
const count = (source, marker) => source.split(marker).length - 1;

const relationsReceiptsPath =
  "crates/modules/rustok-product-relations/src/services/receipts.rs";
const relationsServicePath =
  "crates/modules/rustok-product-relations/src/services/relation_service.rs";
const relationsDtoPath = "crates/modules/rustok-product-relations/src/dto.rs";
const relationsServicesModPath = "crates/modules/rustok-product-relations/src/services/mod.rs";
const relationsLibPath = "crates/modules/rustok-product-relations/src/lib.rs";
const relationsManifestPath = "crates/modules/rustok-product-relations/Cargo.toml";
const relationsAdapterPath =
  "crates/modules/rustok-product-relations/admin/src/transport/native_server_adapter.rs";
const relationsMigrationsPath = "crates/modules/rustok-product-relations/src/migrations/mod.rs";
const relationsTenantKeyCleanupMigrationPath =
  "crates/modules/rustok-product-relations/src/migrations/m20261017_000005_remove_redundant_product_tenant_key.rs";
const bundlesReceiptsPath = "crates/modules/rustok-product-bundles/src/services/receipts.rs";
const bundlesServicePath = "crates/modules/rustok-product-bundles/src/services/bundle_service.rs";
const bundlesDtoPath = "crates/modules/rustok-product-bundles/src/dto.rs";
const bundlesMigrationsPath = "crates/modules/rustok-product-bundles/src/migrations/mod.rs";
const bundlesLocaleMigrationPath =
  "crates/modules/rustok-product-bundles/src/migrations/m20261017_000002_normalize_bundle_translation_locales.rs";
const bundlesIntegrityMigrationPath =
  "crates/modules/rustok-product-bundles/src/migrations/m20261017_000004_enforce_bundle_tenant_integrity.rs";
const bundlesCreateMigrationPath =
  "crates/modules/rustok-product-bundles/src/migrations/m20260916_000001_create_bundles.rs";
const productSqliteIdentityMigrationPath =
  "crates/modules/rustok-product/src/migrations/m20261017_000036_add_sqlite_tenant_identity_keys.rs";
const productWriteTransactionPath =
  "crates/modules/rustok-product/src/services/write_transaction.rs";
const productSchemaWritePortPath =
  "crates/modules/rustok-product/src/catalog_schema_write_port.rs";
const productSchemaAttributesPath =
  "crates/modules/rustok-product/src/services/catalog_schema_service/attributes.rs";
const productTranslationTargetPath = "crates/modules/rustok-product/src/translation_target.rs";
const productVariantTranslationTargetPath =
  "crates/modules/rustok-product/src/variant_translation_target.rs";
const productImageTranslationTargetPath =
  "crates/modules/rustok-product/src/image_translation_target.rs";
const bundlesServicesModPath = "crates/modules/rustok-product-bundles/src/services/mod.rs";
const bundlesLibPath = "crates/modules/rustok-product-bundles/src/lib.rs";
const bundlesManifestPath = "crates/modules/rustok-product-bundles/Cargo.toml";
const bundlesAdapterPath =
  "crates/modules/rustok-product-bundles/admin/src/transport/native_server_adapter.rs";
const outboxIdempotencyPath = "crates/modules/rustok-outbox/src/idempotency.rs";
const graphqlPath = "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs";
const graphqlTypesPath = "crates/modules/rustok-commerce/src/graphql/types.rs";
const productCatalogGraphqlPath = "crates/modules/rustok-commerce/src/graphql/product_catalog.rs";
const relationsGraphqlAdapterPath =
  "crates/modules/rustok-product-relations/admin/src/transport/graphql_adapter.rs";
const bundlesGraphqlAdapterPath =
  "crates/modules/rustok-product-bundles/admin/src/transport/graphql_adapter.rs";
const nextRelationsPath = "apps/next-admin/packages/rustok-product/src/api/relations.ts";
const nextBundlesPath = "apps/next-admin/packages/rustok-product/src/api/bundles.ts";
const boundaryDocPath = "crates/modules/rustok-product/docs/write-boundary.md";
const registryPath = "docs/modules/registry.md";
const auditPath = "docs/audits/product-module-engineering-audit-2026-10-07.md";

for (const relative of [
  relationsReceiptsPath,
  relationsServicePath,
  relationsDtoPath,
  relationsServicesModPath,
  relationsLibPath,
  relationsManifestPath,
  relationsAdapterPath,
  relationsMigrationsPath,
  relationsTenantKeyCleanupMigrationPath,
  bundlesReceiptsPath,
  bundlesServicePath,
  bundlesDtoPath,
  bundlesMigrationsPath,
  bundlesLocaleMigrationPath,
  bundlesIntegrityMigrationPath,
  bundlesCreateMigrationPath,
  productSqliteIdentityMigrationPath,
  productWriteTransactionPath,
  productSchemaWritePortPath,
  productSchemaAttributesPath,
  productTranslationTargetPath,
  productVariantTranslationTargetPath,
  productImageTranslationTargetPath,
  bundlesServicesModPath,
  bundlesLibPath,
  bundlesManifestPath,
  bundlesAdapterPath,
  outboxIdempotencyPath,
  graphqlPath,
  graphqlTypesPath,
  productCatalogGraphqlPath,
  relationsGraphqlAdapterPath,
  bundlesGraphqlAdapterPath,
  nextRelationsPath,
  nextBundlesPath,
  boundaryDocPath,
  registryPath,
  auditPath,
]) {
  if (!fs.existsSync(path.join(root, relative))) failures.push(`missing ${relative}`);
}

if (failures.length > 0) {
  console.error("[verify-product-relations-bundles-idempotency] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const relationsReceipts = read(relationsReceiptsPath);
const relationsService = read(relationsServicePath);
const relationsDto = read(relationsDtoPath);
const relationsServicesMod = read(relationsServicesModPath);
const relationsLib = read(relationsLibPath);
const relationsManifest = read(relationsManifestPath);
const relationsAdapter = read(relationsAdapterPath);
const relationsMigrations = read(relationsMigrationsPath);
const relationsTenantKeyCleanupMigration = read(relationsTenantKeyCleanupMigrationPath);
const relationsTenantKeyCleanupUp = slice(
  relationsTenantKeyCleanupMigration,
  "async fn up",
  "async fn down",
);
const bundlesReceipts = read(bundlesReceiptsPath);
const bundlesService = read(bundlesServicePath);
const bundlesDto = read(bundlesDtoPath);
const bundlesMigrations = read(bundlesMigrationsPath);
const bundlesLocaleMigration = read(bundlesLocaleMigrationPath);
const bundlesIntegrityMigration = read(bundlesIntegrityMigrationPath);
const bundlesCreateMigration = read(bundlesCreateMigrationPath);
const productSqliteIdentityMigration = read(productSqliteIdentityMigrationPath);
const productSqliteIdentityMigrationUp = slice(
  productSqliteIdentityMigration,
  "async fn up",
  "async fn down",
);
const productWriteTransaction = read(productWriteTransactionPath);
const productSchemaWritePort = read(productSchemaWritePortPath);
const productSchemaAttributes = read(productSchemaAttributesPath);
const productTranslationTarget = read(productTranslationTargetPath);
const productVariantTranslationTarget = read(productVariantTranslationTargetPath);
const productImageTranslationTarget = read(productImageTranslationTargetPath);
const productDeferredRollback = slice(
  productWriteTransaction,
  "impl ProductOperationReceipt",
  "tokio::task_local!",
);
const productReceiptScope = slice(
  productWriteTransaction,
  "pub(crate) async fn with_product_operation_receipt",
  "/// Persists only a deterministic Product owner failure",
);
const productFailureReceiptFinalizer = slice(
  productWriteTransaction,
  "pub(crate) async fn fail_product_operation_receipt",
  "/// The receipt operation UUID",
);
const productWriteTransactionDrop = slice(
  productWriteTransaction,
  "impl Drop for ProductWriteTransaction",
  "fn product_index_revision_touch_target",
);
const productReceiptResultRecord = slice(
  productWriteTransaction,
  "pub(crate) fn record_product_operation_result",
  "/// Owns one product write transaction",
);
const productReceiptRollback = slice(
  productWriteTransaction,
  "async fn rollback_product_receipt_failure",
  "/// Completes a Product owner receipt",
);
const productReceiptCompletion = slice(
  productWriteTransaction,
  "async fn complete_product_receipt_and_commit",
  "impl ProductWriteTransaction",
);
const productReceiptResultFailure = slice(
  productReceiptCompletion,
  "let response_json = match response_json {",
  "if let Err(error) = idempotency::complete",
);
const productReceiptCompletionFailure = slice(
  productReceiptCompletion,
  "if let Err(error) = idempotency::complete",
  "transaction.commit().await?;",
);
const bundlesServicesMod = read(bundlesServicesModPath);
const bundlesLib = read(bundlesLibPath);
const bundlesManifest = read(bundlesManifestPath);
const bundlesAdapter = read(bundlesAdapterPath);
const outboxIdempotency = read(outboxIdempotencyPath);
const outboxReceiptCompletion = slice(
  outboxIdempotency,
  "pub async fn complete<C, T>",
  "/// Stores a terminal failure",
);
const graphql = read(graphqlPath);
const graphqlTypes = read(graphqlTypesPath);
const productCatalogGraphql = read(productCatalogGraphqlPath);
const relationsGraphqlAdapter = read(relationsGraphqlAdapterPath);
const bundlesGraphqlAdapter = read(bundlesGraphqlAdapterPath);
const nextRelations = read(nextRelationsPath);
const nextBundles = read(nextBundlesPath);
const boundaryDoc = read(boundaryDocPath);
const registry = read(registryPath);
const audit = read(auditPath);

// ── Product Relation tenant-key cleanup ──────────────────────────────────────────────────────
for (const marker of [
  "mod m20261017_000005_remove_redundant_product_tenant_key;",
  "m20261017_000005_remove_redundant_product_tenant_key::Migration",
  'vec!["m20261017_000003_enforce_relation_positions"]',
]) {
  need(relationsMigrations, marker, "relations tenant-key cleanup registration");
}
for (const marker of [
  "uq_products_tenant_id_for_product_relations",
  "uq_products_tenant_id is missing",
  "DROP CONSTRAINT IF EXISTS fk_product_relations_product",
  "DROP CONSTRAINT IF EXISTS fk_product_relations_related_product",
  "REFERENCES products (tenant_id, id)",
]) {
  need(relationsTenantKeyCleanupUp, marker, "relations tenant-key cleanup migration");
}

// ── Shared receipt ledger, never a parallel one ──────────────────────────────────────────────
for (const [source, label] of [
  [relationsReceipts, "relations receipts"],
  [bundlesReceipts, "bundles receipts"],
]) {
  need(source, "use rustok_outbox::idempotency;", label);
  forbid(source, "DeriveEntityModel", label);
  forbid(source, "owner_operation_receipts", label);
}
for (const [source, label] of [
  [relationsManifest, "relations manifest"],
  [bundlesManifest, "bundles manifest"],
]) {
  need(source, "rustok-outbox.workspace = true", label);
}
for (const marker of [
  "pub fn scoped_caller_idempotency_key(",
  "tenant_id: Uuid,",
  "actor_id: Uuid,",
  "hash_scoped_key_component",
  "hash_scoped_key_component(&mut digest, namespace.as_bytes());",
  "hash_scoped_key_component(&mut digest, tenant_id.as_bytes());",
  "hash_scoped_key_component(&mut digest, actor_id.as_bytes());",
  "hash_scoped_key_component(&mut digest, operation.as_bytes());",
  "digest.update([1]);",
  "hash_scoped_key_component(&mut digest, subject_id.as_bytes());",
  "None => digest.update([0])",
  "hash_scoped_key_component(&mut digest, caller_key.trim().as_bytes());",
]) {
  need(outboxIdempotency, marker, "shared caller-key scoping primitive");
}
for (const marker of [
  "This deployed Product namespace deliberately retains its original digest framing",
  "let mut digest = Sha256::new();",
  "digest.update(tenant_id.as_bytes());",
  "digest.update(user_id.as_bytes());",
  "digest.update(operation.as_bytes());",
  "digest.update(caller_key.trim().as_bytes());",
]) {
  need(productSchemaWritePort, marker, "deployed Product receipt key compatibility");
}
forbid(
  productSchemaWritePort,
  "idempotency::scoped_caller_idempotency_key(",
  "deployed Product receipt key compatibility",
);
for (const [source, namespace, compatibilityMarker, label] of [
  [
    relationsLib,
    'RELATION_CALLER_IDEMPOTENCY_NAMESPACE: &str = "commerce-graphql-relation"',
    "This deployed Product Relation namespace deliberately retains its original digest framing",
    "relations deployed receipt key compatibility",
  ],
  [
    bundlesLib,
    'BUNDLE_CALLER_IDEMPOTENCY_NAMESPACE: &str = "commerce-graphql-bundle"',
    "This deployed Product Bundle namespace deliberately retains its original digest framing",
    "bundles deployed receipt key compatibility",
  ],
]) {
  for (const marker of [
    namespace,
    compatibilityMarker,
    "pub fn scoped_caller_idempotency_key(",
    "let mut digest = Sha256::new();",
    "digest.update(tenant_id.as_bytes());",
    "digest.update(user_id.as_bytes());",
    "digest.update(operation.as_bytes());",
    "digest.update(caller_key.trim().as_bytes());",
  ]) {
    need(source, marker, label);
  }
  forbid(
    source,
    "rustok_outbox::idempotency::scoped_caller_idempotency_key(",
    label,
  );
}
for (const marker of [
  "let update = entity::Entity::update_many()",
  "if update.rows_affected != 1",
  '"outbox.operation_receipt_state_invalid"',
  "owner operation receipt was not processing during failure finalization",
]) {
  need(outboxIdempotency, marker, "shared failure receipt finalization");
}
for (const marker of [
  "if update.rows_affected != 1",
  "return Err(PortError::unavailable(",
  "owner operation receipt was not processing during completion",
]) {
  need(outboxReceiptCompletion, marker, "shared receipt completion finalization");
}

// ── Product owner receipt finalization ────────────────────────────────────────────────────────
// Receipt failure must synchronously undo a staged Product mutation before a caller can reclaim
// the same owner operation. SeaORM's transaction Drop only starts rollback asynchronously.
for (const marker of [
  "async fn complete_product_receipt_and_commit(",
  "async fn rollback_product_receipt_failure(",
  "return complete_product_receipt_and_commit(transaction, receipt).await;",
  "receipt_completion_failure_rolls_back_staged_product_mutation",
  "receipt_result_encoding_failure_rolls_back_staged_product_mutation",
  "missing_receipt_result_rolls_back_staged_product_mutation",
]) {
  need(productWriteTransaction, marker, "product owner receipt completion");
}
for (const marker of [
  "let _ = PRODUCT_OPERATION_RECEIPT.try_with(|receipt| {",
  "let response_json = serde_json::to_value(value).map_err(|error| error.to_string());",
  "*slot = Some(response_json);",
  "poisoned.into_inner()",
]) {
  need(productReceiptResultRecord, marker, "product owner receipt result recording");
}
forbid(
  productReceiptResultRecord,
  "CommerceResult",
  "product owner receipt result recording",
);
need(
  productReceiptRollback,
  "transaction.rollback().await",
  "product owner receipt rollback",
);
for (const marker of [
  "return rollback_product_receipt_failure(transaction, receipt.lease, error).await;",
  "transaction.commit().await?;",
]) {
  need(productReceiptCompletion, marker, "product owner receipt completion");
}
need(
  productReceiptCompletionFailure,
  "return rollback_product_receipt_failure(\n            transaction,",
  "product owner receipt completion failure",
);
need(
  productReceiptResultFailure,
  "return rollback_product_receipt_failure(transaction, receipt.lease, error).await;",
  "product owner receipt result failure",
);
forbid(
  productReceiptCompletion,
  "drop(transaction);",
  "product owner receipt completion",
);

// Owner methods can fail before their success-only `commit` call. A receipt-bound wrapper must
// take ownership of that live transaction on Drop and await rollback after the task-local scope
// ends, before any independent terminal failure receipt can be persisted.
need(
  productWriteTransaction,
  "deferred_rollbacks: Arc<Mutex<Vec<DatabaseTransaction>>>",
  "product owner deferred rollback",
);
for (const marker of [
  "async fn rollback_deferred_transactions(&self)",
  "transaction.rollback().await",
]) {
  need(productDeferredRollback, marker, "product owner deferred rollback");
}
for (const marker of [
  "F: Future<Output = Result<T, E>>",
  "E: From<CommerceError>",
  "let rolled_back = receipt",
  ".rollback_deferred_transactions()",
  ".map_err(E::from)?",
  "product owner write returned success without committing its transaction",
]) {
  need(productReceiptScope, marker, "product owner receipt scope rollback fence");
}
for (const marker of [
  "transaction: Option<DatabaseTransaction>",
  "impl Drop for ProductWriteTransaction",
  "receipt.defer_rollback(transaction);",
  "receipt_scoped_early_error_awaits_owner_transaction_rollback",
]) {
  need(productWriteTransaction, marker, "product owner early-error rollback");
}
need(
  productWriteTransactionDrop,
  "receipt.defer_rollback(transaction);",
  "product owner transaction Drop rollback handoff",
);
for (const marker of [
  "if error.retryable {",
  "return Ok(());",
  "idempotency::fail(database, lease, error).await",
]) {
  need(productFailureReceiptFinalizer, marker, "product retryable receipt finalization");
}
for (const [source, label] of [
  [productTranslationTarget, "Product translation-target receipt finalizer"],
  [productVariantTranslationTarget, "Product Variant translation-target receipt finalizer"],
  [productImageTranslationTarget, "Product Image translation-target receipt finalizer"],
  [productSchemaAttributes, "Product schema receipt finalizer"],
]) {
  need(source, "fail_product_operation_receipt(", label);
  forbid(source, "idempotency::fail(", label);
}

// A terminal owner error is only replay-safe after its failure receipt was persisted. Returning
// it after a lost lease or a finalization outage falsely claims a durable outcome; the retryable
// receipt error must win in that case.
for (const [source, label] of [
  [productTranslationTarget, "Product translation-target receipt finalizer propagation"],
  [productVariantTranslationTarget, "Product Variant translation-target receipt finalizer propagation"],
  [productImageTranslationTarget, "Product Image translation-target receipt finalizer propagation"],
]) {
  need(source, "self.fail_receipt(lease, error).await?;", label);
}
const productSchemaFailureFinalizer = slice(
  productSchemaWritePort,
  "async fn finish_receipted_schema_write<T>",
  "fn decode_schema_receipt",
);
need(
  productSchemaFailureFinalizer,
  "return Err(schema_receipt_error(context, operation, receipt_error));",
  "Product schema receipt finalizer propagation",
);
need(
  boundaryDoc,
  "receipt error—not the original terminal domain error",
  "write boundary doc",
);

// ── Relations receipt protocol ───────────────────────────────────────────────────────────────
for (const marker of [
  'pub const PRODUCT_RELATION_OWNER_SLUG: &str = "product_relations";',
  'pub const CREATE_RELATION_OPERATION: &str = "create_relation";',
  "pub struct ProductRelationCommandContext<'a>",
  "idempotency::OwnerOperationScope::Tenant(context.tenant_id)",
  "idempotency::admit(",
  "idempotency::Admission::Run(lease) => lease,",
  "idempotency::Admission::Replay(value) => return decode_relation_receipt(value),",
  "idempotency::fail(",
  "async fn complete_relation_operation<T: serde::Serialize>(",
  "if let Err(receipt_error) = idempotency::complete(&txn, lease, response).await {",
  "rollback_relation_operation(txn, context, lease).await?;",
  "async fn rollback_relation_operation(",
  "txn.rollback().await",
  "async fn finish_failed_relation_operation(",
  ") -> Result<(), PortError>",
  "create_relation_in_tx(&txn, context.tenant_id, input)",
  "pub fn relation_command_error(error: &ProductRelationError) -> PortError",
]) {
  need(relationsReceipts, marker, "relations receipts");
}
const relationReceiptCompletion = slice(
  relationsReceipts,
  "async fn complete_relation_operation<T: serde::Serialize>(",
  "/// Rolls back the owner mutation",
);
for (const marker of [
  "if let Err(receipt_error) = idempotency::complete(&txn, lease, response).await {",
  "rollback_relation_operation(txn, context, lease).await?;",
  "txn.commit().await?;",
]) {
  need(relationReceiptCompletion, marker, "relations successful receipt completion");
}
forbid(relationReceiptCompletion, "drop(txn);", "relations successful receipt completion");
need(
  relationsService,
  "pub(crate) async fn create_relation_in_tx(",
  "relations service transaction core",
);
need(relationsService, "pub(crate) fn database(&self) -> &DatabaseConnection", "relations service");
for (const marker of [
  "pub const MAX_RELATION_TYPE_LEN: usize = 32;",
  "pub fn storage_key(&self) -> Result<String, ProductRelationError>",
  "normalized.chars().count() > MAX_RELATION_TYPE_LEN",
  "normalized.contains('\\0')",
]) {
  need(relationsDto, marker, "relations type storage validation");
}
for (const marker of [
  "let rel_type_str = relation_type.storage_key()?;",
  "relation_type.storage_key()?",
  "validate_relation_metadata(&metadata)?;",
  "relation metadata cannot contain a NUL character",
]) {
  need(relationsService, marker, "relations service storage validation");
}
for (const marker of [
  "let UpdateProductRelationInput {",
  "if let Some(metadata) = &metadata {",
  "validate_relation_metadata(metadata)?;",
  "let relation = lock_relation_for_mutation(txn, tenant_id, relation_id).await?;",
]) {
  need(relationsService, marker, "relation update validation before mutation");
}
const updateRelationMutation = slice(
  relationsService,
  "pub(crate) async fn update_relation_in_tx(",
  "/// Deletes one relation",
);
if (
  updateRelationMutation.indexOf("validate_relation_metadata(metadata)?;") >
  updateRelationMutation.indexOf("normalize_relation_group_in_tx(")
) {
  failures.push("relation update validation must precede position normalization writes");
}

for (const marker of [
  "async fn lock_relation_for_mutation(",
  "lock_product_for_update(txn, tenant_id, relation.product_id).await?;",
  "let relation = lock_relation_for_mutation(txn, tenant_id, relation_id).await?;",
  "let model = lock_relation_for_mutation(txn, tenant_id, relation_id).await?;",
  "if delete.rows_affected != 1",
]) {
  need(relationsService, marker, "relation mutation lock and delete result");
}
const relationReceiptCommands = [
  [
    "create_relation_idempotent",
    "pub async fn create_relation_idempotent(",
    "pub async fn update_relation_idempotent(",
    "admit_relation_operation(self, context, CREATE_RELATION_OPERATION, &request)",
    "create_relation_in_tx(&txn, context.tenant_id, input)",
    "complete_relation_operation(txn, context, lease, &relation).await?;",
  ],
  [
    "update_relation_idempotent",
    "pub async fn update_relation_idempotent(",
    "pub async fn delete_relation_idempotent(",
    "admit_relation_operation(self, context, UPDATE_RELATION_OPERATION, &request)",
    "update_relation_in_tx(&txn, context.tenant_id, relation_id, input)",
    "complete_relation_operation(txn, context, lease, &relation).await?;",
  ],
  [
    "delete_relation_idempotent",
    "pub async fn delete_relation_idempotent(",
    "pub async fn reorder_relations_idempotent(",
    "admit_relation_operation(self, context, DELETE_RELATION_OPERATION, &request)",
    "delete_relation_in_tx(&txn, context.tenant_id, relation_id)",
    "complete_relation_operation(txn, context, lease, &()).await?;",
  ],
  [
    "reorder_relations_idempotent",
    "pub async fn reorder_relations_idempotent(",
    "async fn admit_relation_operation",
    "admit_relation_operation(self, context, REORDER_RELATIONS_OPERATION, &request)",
    "reorder_relations_in_tx(&txn, context.tenant_id, input)",
    "complete_relation_operation(txn, context, lease, &relations).await?;",
  ],
];
for (const [name, start, end, admission, write, completion] of relationReceiptCommands) {
  const command = slice(relationsReceipts, start, end);
  for (const marker of [
    admission,
    write,
    completion,
    "rollback_relation_operation(txn, context, lease).await?;",
    "finish_failed_relation_operation(self, context, lease, &error).await?;",
  ]) {
    need(command, marker, `${name} receipt command`);
  }
  forbid(command, "drop(txn);", `${name} receipt command`);
}
const relationsPortMethod = slice(
  relationsService,
  "    async fn create_relation(",
  "    async fn update_relation(",
);
need(
  relationsPortMethod,
  "let relation = create_relation_in_tx(&txn, tenant_id, input).await?;",
  "relations port method",
);
need(relationsPortMethod, "txn.commit().await?;", "relations port method");
for (const marker of [
  "ProductRelationCommandContext",
  "ProductRelationCommandError",
  "relation_command_error",
]) {
  need(relationsServicesMod, marker, "relations services module");
  need(relationsLib, marker, "relations crate root");
}

// ── Bundle receipt protocol ──────────────────────────────────────────────────────────────────
for (const marker of [
  'pub const PRODUCT_BUNDLE_OWNER_SLUG: &str = "product_bundles";',
  'pub const CREATE_BUNDLE_OPERATION: &str = "create_bundle";',
  'pub const ADD_BUNDLE_ITEM_OPERATION: &str = "add_bundle_item";',
  "pub struct BundleCommandContext<'a>",
  "idempotency::admit(",
  "idempotency::fail(service.database(), lease, &port_error)",
  "async fn complete_bundle_operation<T: serde::Serialize>(",
  "if let Err(receipt_error) = idempotency::complete(&txn, lease, response).await {",
  "rollback_bundle_operation(txn, context, lease).await?;",
  "async fn rollback_bundle_operation(",
  "txn.rollback().await",
  "async fn finish_failed_bundle_operation(",
  ") -> Result<(), PortError>",
  "create_bundle_in_tx(&txn, context.tenant_id, input)",
  "add_bundle_item_in_tx(&txn, context.tenant_id, bundle_id, item)",
  "pub fn bundle_command_error(error: &BundleError) -> PortError",
  '"product_bundle.slug_conflict"',
  '"product_bundle.item_not_found"',
]) {
  need(bundlesReceipts, marker, "bundles receipts");
}
const bundleReceiptCompletion = slice(
  bundlesReceipts,
  "async fn complete_bundle_operation<T: serde::Serialize>(",
  "/// Rolls back the owner mutation",
);
for (const marker of [
  "if let Err(receipt_error) = idempotency::complete(&txn, lease, response).await {",
  "rollback_bundle_operation(txn, context, lease).await?;",
  "txn.commit().await?;",
]) {
  need(bundleReceiptCompletion, marker, "bundles successful receipt completion");
}
forbid(bundleReceiptCompletion, "drop(txn);", "bundles successful receipt completion");
for (const marker of [
  "pub(crate) async fn create_bundle_in_tx(",
  "pub(crate) async fn add_bundle_item_in_tx(",
  "pub(crate) fn database(&self) -> &DatabaseConnection",
  "fn map_unique_constraint_conflict(error: DbErr) -> BundleError",
  "Some(sea_orm::SqlErr::UniqueConstraintViolation(_))",
  "BundleError::ConcurrentWriteConflict",
  ".map_err(map_unique_constraint_conflict)?",
]) {
  need(bundlesService, marker, "bundles service transaction core");
}
for (const marker of [
  "fn normalize_bundle_slug(value: String) -> BundleResult<String>",
  "fn normalize_bundle_translations(",
  "normalize_locale_tag(&translation.locale)",
  "normalize_bundle_translations(translations)?",
  "fn validate_decimal_storage(",
  "fn json_contains_nul(",
  "Bundle item positions exceed the supported range",
  ".order_by_desc(BundleItemColumn::Position)",
]) {
  need(bundlesService, marker, "bundles storage validation");
}
for (const marker of [
  "let delete = Bundle::delete_by_id(bundle.id).exec(txn).await?;",
  "let delete = BundleItem::delete_by_id(item.id).exec(txn).await?;",
  "if delete.rows_affected != 1",
]) {
  need(bundlesService, marker, "bundle destructive mutation affected-row checks");
}

for (const marker of [
  "pub struct BundleTranslationUpdateInput",
  "fn deserialize_optional_optional<'de, T, D>(",
  `#[serde(
        default,
        deserialize_with = "deserialize_optional_optional",
        skip_serializing_if = "Option::is_none"
    )]
    pub bundle_product_id: Option<Option<Uuid>>`,
  `#[serde(
        default,
        deserialize_with = "deserialize_optional_optional",
        skip_serializing_if = "Option::is_none"
    )]
    pub metadata: Option<Option<serde_json::Value>>`,
]) {
  need(bundlesDto, marker, "bundles update patch DTO");
}
for (const marker of [
  "fn normalize_bundle_translation_updates(",
  "A new Bundle translation must provide a name",
  "if let Some(metadata) = metadata",
  "let filter = normalize_bundle_filter(filter)?;",
  "build_locale_candidates([locale, Some(PLATFORM_FALLBACK_LOCALE)], true);",
  "locale_tags_match(&translation.locale, candidate)",
  "translations.sort_by(|left, right|",
]) {
  need(bundlesService, marker, "bundles update/read normalization");
}

for (const marker of [
  "m20261017_000002_normalize_bundle_translation_locales",
  'vec!["m20260916_000001_create_bundles"]',
]) {
  need(bundlesMigrations, marker, "bundles locale migration registration");
}
for (const marker of [
  "m20261017_000004_enforce_bundle_tenant_integrity",
  "m20261017_000036_add_sqlite_tenant_identity_keys",
]) {
  need(bundlesMigrations, marker, "bundles tenant-integrity migration registration");
}
for (const marker of [
  "ALTER TABLE product_bundle_translations ALTER COLUMN locale TYPE VARCHAR(32)",
  "normalize_locale_tag(&row.locale)",
  "BTreeMap::<(Uuid, String), Uuid>::new()",
  "?1 WHERE id = ?2",
]) {
  need(bundlesLocaleMigration, marker, "bundles locale migration");
}
for (const marker of [
  "fn use_transaction(&self) -> Option<bool>",
  "ADD COLUMN IF NOT EXISTS tenant_id UUID",
  "CREATE TABLE product_bundles_tenant_integrity",
  "CREATE TABLE product_bundle_items_tenant_integrity",
  "REFERENCES product_bundles_tenant_integrity (tenant_id, id)",
  "DROP TABLE product_bundle_items;",
  "async fn downgrade_postgres(",
  "async fn downgrade_sqlite(",
  "A Product may own at most one Product Bundle per tenant",
]) {
  need(bundlesIntegrityMigration, marker, "bundles tenant-integrity upgrade migration");
}
for (const marker of [
  "FOREIGN KEY (tenant_id, product_id, variant_id)",
  "REFERENCES product_variants (tenant_id, product_id, id)",
  "variant.product_id IS DISTINCT FROM item.product_id",
]) {
  need(bundlesIntegrityMigration, marker, "bundles Variant-to-Product integrity migration");
}
for (const marker of [
  "uq_product_variants_tenant_product_id",
  "UNIQUE (tenant_id, product_id, id)",
]) {
  need(productSqliteIdentityMigrationUp, marker, "Product Variant composite parent key");
}
need(
  bundlesCreateMigration,
  "bundle_id UUID NOT NULL REFERENCES product_bundles(id) ON DELETE CASCADE",
  "bundles historical create migration",
);
forbid(
  bundlesCreateMigration,
  "fk_product_bundle_items_bundle_tenant",
  "bundles historical create migration",
);
need(
  bundlesReceipts,
  '"product_bundle.concurrent_conflict"',
  "bundles receipt conflict mapping",
);
const bundleReceiptCommands = [
  [
    "create_bundle_idempotent",
    "pub async fn create_bundle_idempotent(",
    "pub async fn add_bundle_item_idempotent(",
    "admit_bundle_operation(self, context, CREATE_BUNDLE_OPERATION, &request)",
    "create_bundle_in_tx(&txn, context.tenant_id, input)",
    "complete_bundle_operation(txn, context, lease, &bundle).await?;",
  ],
  [
    "add_bundle_item_idempotent",
    "pub async fn add_bundle_item_idempotent(",
    "pub async fn update_bundle_idempotent(",
    "admit_bundle_operation(self, context, ADD_BUNDLE_ITEM_OPERATION, &request)",
    "add_bundle_item_in_tx(&txn, context.tenant_id, bundle_id, item)",
    "complete_bundle_operation(txn, context, lease, &created_item).await?;",
  ],
  [
    "update_bundle_idempotent",
    "pub async fn update_bundle_idempotent(",
    "pub async fn delete_bundle_idempotent(",
    "admit_bundle_operation(self, context, UPDATE_BUNDLE_OPERATION, &request)",
    "update_bundle_in_tx(&txn, context.tenant_id, bundle_id, input)",
    "complete_bundle_operation(txn, context, lease, &bundle).await?;",
  ],
  [
    "delete_bundle_idempotent",
    "pub async fn delete_bundle_idempotent(",
    "pub async fn remove_bundle_item_idempotent(",
    "admit_bundle_operation(self, context, DELETE_BUNDLE_OPERATION, &request)",
    "delete_bundle_in_tx(&txn, context.tenant_id, bundle_id)",
    "complete_bundle_operation(txn, context, lease, &()).await?;",
  ],
  [
    "remove_bundle_item_idempotent",
    "pub async fn remove_bundle_item_idempotent(",
    "async fn admit_bundle_operation",
    "admit_bundle_operation(",
    "remove_bundle_item_in_tx(&txn, context.tenant_id, bundle_id, item_id)",
    "complete_bundle_operation(txn, context, lease, &()).await?;",
  ],
];
for (const [name, start, end, admission, write, completion] of bundleReceiptCommands) {
  const command = slice(bundlesReceipts, start, end);
  for (const marker of [
    admission,
    write,
    completion,
    "rollback_bundle_operation(txn, context, lease).await?;",
    "finish_failed_bundle_operation(self, context, lease, &error).await?;",
  ]) {
    need(command, marker, `${name} receipt command`);
  }
  forbid(command, "drop(txn);", `${name} receipt command`);
}
need(
  bundlesService,
  "let bundle = create_bundle_in_tx(&txn, tenant_id, input).await?;",
  "bundles port method",
);
need(
  bundlesService,
  "let created_item = add_bundle_item_in_tx(&txn, tenant_id, bundle_id, item).await?;",
  "bundles port method",
);
for (const marker of ["BundleCommandContext", "BundleCommandError", "bundle_command_error"]) {
  need(bundlesServicesMod, marker, "bundles services module");
  need(bundlesLib, marker, "bundles crate root");
}

// ── Every mounted write ingress uses the owner receipt contract ───────────────────────────────
// Keep the GraphQL slices explicit: adding a write without a required caller key or changing an
// existing write back to a plain service call must fail this repository guard.
const relationGraphqlWrites = [
  ["add_product_relation", "async fn add_product_relation(", "async fn remove_product_relation(", ".create_relation_idempotent(", "None,"],
  ["remove_product_relation", "async fn remove_product_relation(", "async fn reorder_product_relations(", ".delete_relation_idempotent(", "Some(id),"],
  ["reorder_product_relations", "async fn reorder_product_relations(", "async fn create_brand(", ".reorder_relations_idempotent(", "Some(product_id),"],
];
for (const [operation, start, end, receiptCall, subject] of relationGraphqlWrites) {
  const mutation = slice(graphql, start, end);
  for (const marker of [
    "idempotency_key: String,",
    'validate_scoped_idempotency_key(&idempotency_key, "Product relation")?',
    `let operation = "${operation}";`,
    "scoped_relation_caller_key(",
    subject,
    "ProductRelationCommandContext::new(",
    receiptCall,
    "relation_command_graphql_error(operation, error)",
  ]) {
    need(mutation, marker, `${operation} GraphQL mutation`);
  }
  forbid(mutation, "idempotency_key: Option<String>", `${operation} GraphQL mutation`);
}

const bundleGraphqlWrites = [
  ["create_bundle", "async fn create_bundle(", "async fn update_bundle(", ".create_bundle_idempotent(", "None,"],
  ["update_bundle", "async fn update_bundle(", "async fn delete_bundle(", ".update_bundle_idempotent(", "Some(id),"],
  ["delete_bundle", "async fn delete_bundle(", "async fn add_bundle_item(", ".delete_bundle_idempotent(", "Some(id),"],
  ["add_bundle_item", "async fn add_bundle_item(", "async fn remove_bundle_item(", ".add_bundle_item_idempotent(", "Some(bundle_id),"],
  ["remove_bundle_item", "async fn remove_bundle_item(", "async fn create_product_attribute(", ".remove_bundle_item_idempotent(", "Some(bundle_id),"],
];
for (const [operation, start, end, receiptCall, subject] of bundleGraphqlWrites) {
  const mutation = slice(graphql, start, end);
  for (const marker of [
    "idempotency_key: String,",
    'validate_scoped_idempotency_key(&idempotency_key, "Product bundle")?',
    `let operation = "${operation}";`,
    "scoped_bundle_caller_key(",
    subject,
    "BundleCommandContext::new(",
    receiptCall,
    "bundle_command_graphql_error(operation, error)",
  ]) {
    need(mutation, marker, `${operation} GraphQL mutation`);
  }
  forbid(mutation, "idempotency_key: Option<String>", `${operation} GraphQL mutation`);
}
const updateBundleMutation = slice(
  graphql,
  "async fn update_bundle(",
  "async fn delete_bundle(",
);
for (const marker of [
  "pub bundle_product_id: MaybeUndefined<Uuid>",
  "fn bundle_product_id_update(value: MaybeUndefined<Uuid>) -> Option<Option<Uuid>>",
  "match graphql_patch(value)",
  "Patch::Keep => None",
  "Patch::Clear => Some(None)",
  "Patch::Set(product_id) => Some(Some(product_id))",
]) {
  need(
    marker.startsWith("pub bundle_product_id") ? graphqlTypes : graphql,
    marker,
    "Bundle GraphQL owner tri-state contract",
  );
}
need(
  updateBundleMutation,
  "bundle_product_id: bundle_product_id_update(input.bundle_product_id)",
  "update_bundle GraphQL mutation",
);
forbid(
  updateBundleMutation,
  "bundle_product_id: input.bundle_product_id.map(Some)",
  "update_bundle GraphQL mutation",
);

const productRelationsQuery = slice(
  productCatalogGraphql,
  "async fn product_relations(",
  "async fn brand(",
);
for (const marker of [
  'require_module_enabled(ctx, "product_relations").await?;',
  "require_commerce_permission(",
  "&[Permission::PRODUCTS_READ]",
  "Permission denied: products:read required",
]) {
  need(productRelationsQuery, marker, "product_relations GraphQL query authorization");
}

for (const [name, start, end] of [
  ["bundle", "async fn bundle(", "async fn bundle_by_slug("],
  ["bundle_by_slug", "async fn bundle_by_slug(", "async fn bundles("],
  ["bundles", "async fn bundles(", "async fn product_bundles("],
  ["product_bundles", "async fn product_bundles(", "__end_of_product_catalog__"],
]) {
  const query = slice(productCatalogGraphql, start, end);
  need(
    query,
    "require_product_bundle_read(ctx)?;",
    `${name} GraphQL query authorization`,
  );
  forbid(
    query,
    ".map_err(|e| async_graphql::Error::new(e.to_string()))?;",
    `${name} GraphQL query error boundary`,
  );
}
for (const marker of [
  "fn require_product_bundle_read(ctx: &Context<'_>) -> Result<()>",
  "&[Permission::PRODUCTS_READ]",
  "fn bundle_query_error(",
  "Bundle data is temporarily unavailable",
]) {
  need(productCatalogGraphql, marker, "bundle GraphQL read safety");
}
for (const marker of [
  "pub description: MaybeUndefined<String>",
  "pub metadata: MaybeUndefined<Json<serde_json::Value>>",
]) {
  need(graphqlTypes, marker, "Bundle GraphQL update patch input");
}
for (const marker of [
  "fn bundle_metadata_update(",
  "fn bundle_translation_updates(",
  "metadata: bundle_metadata_update(input.metadata)",
  "input.name,",
  "input.description,",
]) {
  need(graphql, marker, "Bundle GraphQL update patch mapping");
}

for (const marker of [
  "impl TryFrom<rustok_product_relations::dto::RelationType> for GqlRelationType",
  "custom relation types are unavailable through the GraphQL enum contract",
  "impl TryFrom<rustok_product_relations::dto::ProductRelationDto> for GqlProductRelation",
]) {
  need(graphqlTypes, marker, "GraphQL relation type projection");
}
forbid(
  graphqlTypes,
  "RelationType::Custom(_) => GqlRelationType::Related",
  "GraphQL relation type projection",
);
for (const [name, start, end, marker] of [
  [
    "add_product_relation",
    "async fn add_product_relation(",
    "async fn remove_product_relation(",
    "relation_type: input.relation_type.into(),",
  ],
  [
    "reorder_product_relations",
    "async fn reorder_product_relations(",
    "async fn create_brand(",
    "relation_type: relation_type.into(),",
  ],
]) {
  need(
    slice(graphql, start, end),
    marker,
    `${name} GraphQL relation type ingress`,
  );
}
for (const marker of [
  "fn relation_type_to_gql(rtype: &str) -> Result<&'static str, String>",
  "RelationType::from_str(rtype)",
  "RelationType::Custom(_) => {",
  'Err("Custom product relation types require the native transport".to_owned())',
  ".transpose()?",
]) {
  need(relationsGraphqlAdapter, marker, "relations remote graphql adapter type mapping");
}
forbid(
  relationsGraphqlAdapter,
  '_ => "RELATED"',
  "relations remote graphql adapter type mapping",
);

for (const marker of [
  "scoped_caller_idempotency_key as scoped_relation_caller_key",
  "scoped_caller_idempotency_key as scoped_bundle_caller_key",
  '"outbox.operation_receipt_conflict"',
  '"IDEMPOTENCY_KEY_CONFLICT"',
  "outbox.operation_receipt_in_progress",
]) {
  need(graphql, marker, "catalog write ingress");
}
forbid(graphql, "fn scoped_catalog_operation_key(", "catalog write ingress");
for (const plainCall of [
  ".create_relation(",
  ".delete_relation(",
  ".reorder_relations(",
  ".create_bundle(",
  ".update_bundle(",
  ".delete_bundle(",
  ".add_bundle_item(",
  ".remove_bundle_item(",
]) {
  forbid(graphql, plainCall, "catalog write ingress");
}

// ── Native admin ingress must derive exactly the same owner key as GraphQL ───────────────────
for (const [source, helper, ownerHelper, label] of [
  [relationsAdapter, "scoped_relation_command_key(", "rustok_product_relations::scoped_caller_idempotency_key(", "relations native adapter"],
  [bundlesAdapter, "scoped_bundle_command_key(", "rustok_product_bundles::scoped_caller_idempotency_key(", "bundles native adapter"],
]) {
  need(source, helper, label);
  need(source, ownerHelper, label);
  need(source, "MAX_CALLER_IDEMPOTENCY_KEY_LENGTH", label);
  need(source, "caller_key.trim()", label);
  need(source, "auth.tenant_id.is_nil() || auth.user_id.is_nil() || tenant.id.is_nil()", label);
  forbid(source, "let _ = idempotency_key;", label);
  forbid(source, "idempotency_key.as_str()", label);
}
for (const marker of [
  '"add_product_relation"',
  '"remove_product_relation"',
  '"reorder_product_relations"',
  "create_relation_idempotent(",
  "delete_relation_idempotent(",
  "reorder_relations_idempotent(",
  "ProductRelationCommandContext::new(",
  "relation_command_error_copy(&error)",
  "fn relation_error_copy(",
  "fn relation_receipt_error_copy(",
  "ProductRelationCommandError::Receipt(error) => relation_receipt_error_copy(error)",
  "outbox.operation_receipt_in_progress",
  "still being processed",
  "Product relation operation is temporarily unavailable",
  "ProductRelationError::Database(db_err)",
  "Internal database error",
  "Internal processing error",
  "ProductRelationError::InvalidRelationType(_)",
  "Invalid product relation request",
  ".map(RelationType::from_str)",
  ".transpose()",
]) {
  need(relationsAdapter, marker, "relations native adapter");
}
forbid(
  relationsAdapter,
  "ProductRelationCommandError::Domain(other) => other.to_string()",
  "relations native adapter",
);
forbid(
  relationsAdapter,
  "error.message.clone()",
  "relations native adapter",
);
for (const marker of [
  '"create_bundle"',
  '"update_bundle"',
  '"delete_bundle"',
  '"add_bundle_item"',
  '"remove_bundle_item"',
  "BundleAdminCommand::Create { .. } | BundleAdminCommand::Delete { .. }",
  "Permission::PRODUCTS_MANAGE",
  "create_bundle_idempotent(",
  "update_bundle_idempotent(",
  "delete_bundle_idempotent(",
  "add_bundle_item_idempotent(",
  "remove_bundle_item_idempotent(",
  "BundleCommandContext::new(",
  "bundle_command_error_copy(&error)",
  "fn bundle_error_copy(",
  "fn bundle_receipt_error_copy(",
  "BundleCommandError::Receipt(error) => bundle_receipt_error_copy(error)",
  "outbox.operation_receipt_in_progress",
  "still being processed",
  "Product bundle operation is temporarily unavailable",
  "BundleError::Database(db_err)",
  "Internal database error",
  "Internal processing error",
  "BundleError::InvalidInput(_)",
  "Invalid bundle request",
  "async fn bundle_detail_native(",
  "ensure_tenant(&auth, &tenant, Permission::PRODUCTS_READ)?;",
]) {
  need(bundlesAdapter, marker, "bundles native adapter");
}
forbid(
  bundlesAdapter,
  "BundleCommandError::Domain(error) => error.to_string()",
  "bundles native adapter",
);
forbid(
  bundlesAdapter,
  "error.message.clone()",
  "bundles native adapter",
);
for (const [source, label, plainCalls] of [
  [relationsAdapter, "relations native adapter", [".create_relation(", ".delete_relation(", ".reorder_relations("]],
  [bundlesAdapter, "bundles native adapter", [".create_bundle(", ".update_bundle(", ".delete_bundle(", ".add_bundle_item(", ".remove_bundle_item("]],
]) {
  for (const plainCall of plainCalls) forbid(source, plainCall, label);
}

// ── Remote GraphQL adapters preserve the same caller key and command input ───────────────────
for (const marker of [
  "mutation ProductAdminAddRelation($idempotencyKey: String!, $input: AddProductRelationInput!) {",
  "addProductRelation(idempotencyKey: $idempotencyKey, input: $input) {",
  "mutation ProductAdminRemoveRelation($idempotencyKey: String!, $id: UUID!) {",
  "removeProductRelation(idempotencyKey: $idempotencyKey, id: $id)",
  "mutation ProductAdminReorderRelations($idempotencyKey: String!, $productId: UUID!, $relationType: GqlRelationType!, $orderedRelationIds: [UUID!]!) {",
  "reorderProductRelations(idempotencyKey: $idempotencyKey, productId: $productId, relationType: $relationType, orderedRelationIds: $orderedRelationIds) {",
  "idempotency_key: String,",
  "#[serde(rename = \"idempotencyKey\")]",
  "metadata: Option<serde_json::Value>",
  "metadata: draft.metadata,",
]) {
  need(relationsGraphqlAdapter, marker, "relations remote graphql adapter");
}
forbid(relationsGraphqlAdapter, "_idempotency_key", "relations remote graphql adapter");
for (const marker of [
  "query BundleAdminDirectory($page: Int, $perPage: Int, $search: String, $status: String, $bundleType: String) {",
  "page: $page",
  "perPage: $perPage",
  "filter: { search: $search, status: $status, bundleType: $bundleType }",
  "items { id }",
  "translations {",
  "translations: record",
  "items_count: i.items.len() as i64",
  "mutation BundleAdminCreate($idempotencyKey: String!, $input: CreateBundleInputGql!) {",
  "createBundle(idempotencyKey: $idempotencyKey, input: $input) {",
  "mutation BundleAdminUpdate($idempotencyKey: String!, $id: UUID!, $input: UpdateBundleInputGql!) {",
  "updateBundle(idempotencyKey: $idempotencyKey, id: $id, input: $input) {",
  "mutation BundleAdminDelete($idempotencyKey: String!, $id: UUID!) {",
  "deleteBundle(idempotencyKey: $idempotencyKey, id: $id)",
  "mutation BundleAdminAddItem($idempotencyKey: String!, $bundleId: UUID!, $item: BundleItemInputGql!) {",
  "addBundleItem(idempotencyKey: $idempotencyKey, bundleId: $bundleId, item: $item) {",
  "mutation BundleAdminRemoveItem($idempotencyKey: String!, $bundleId: UUID!, $itemId: UUID!) {",
  "removeBundleItem(idempotencyKey: $idempotencyKey, bundleId: $bundleId, itemId: $itemId)",
  "idempotency_key: String,",
  "#[serde(rename = \"idempotencyKey\")]",
]) {
  need(bundlesGraphqlAdapter, marker, "bundles remote graphql adapter");
}
forbid(
  bundlesGraphqlAdapter,
  '#[serde(rename = "itemsCount"',
  "bundles remote graphql adapter",
);
forbid(
  bundlesGraphqlAdapter,
  "filter: { page: $page, perPage: $perPage,",
  "bundles remote graphql adapter",
);
forbid(bundlesGraphqlAdapter, "_idempotency_key", "bundles remote graphql adapter");

// ── FFA Next admin clients preserve caller-owned retry keys and transport optional values ──────
for (const marker of [
  "mutation ProductAdminAddRelation($idempotencyKey: String!, $input: AddProductRelationInput!) {",
  "addProductRelation(idempotencyKey: $idempotencyKey, input: $input) {",
  "mutation ProductAdminRemoveRelation($idempotencyKey: String!, $id: UUID!) {",
  "removeProductRelation(idempotencyKey: $idempotencyKey, id: $id)",
  "mutation ProductAdminReorderRelations($idempotencyKey: String!, $productId: UUID!, $relationType: GqlRelationType!, $orderedRelationIds: [UUID!]!) {",
  "reorderProductRelations(idempotencyKey: $idempotencyKey, productId: $productId, relationType: $relationType, orderedRelationIds: $orderedRelationIds) {",
  "input: AddProductRelationInput,\n  idempotencyKey = crypto.randomUUID()",
  "id: string,\n  idempotencyKey = crypto.randomUUID()",
  "orderedRelationIds: string[],\n  idempotencyKey = crypto.randomUUID()",
]) {
  need(nextRelations, marker, "next relations client");
}
for (const marker of [
  "mutation CreateBundle($idempotencyKey: String!, $input: CreateBundleInputGql!, $locale: String) {",
  "createBundle(idempotencyKey: $idempotencyKey, input: $input, locale: $locale) {",
  "mutation UpdateBundle($idempotencyKey: String!, $id: UUID!, $input: UpdateBundleInputGql!, $locale: String) {",
  "updateBundle(idempotencyKey: $idempotencyKey, id: $id, input: $input, locale: $locale) {",
  "mutation DeleteBundle($idempotencyKey: String!, $id: UUID!) {",
  "deleteBundle(idempotencyKey: $idempotencyKey, id: $id)",
  "mutation AddBundleItem($idempotencyKey: String!, $bundleId: UUID!, $item: BundleItemInputGql!) {",
  "addBundleItem(idempotencyKey: $idempotencyKey, bundleId: $bundleId, item: $item) {",
  "mutation RemoveBundleItem($idempotencyKey: String!, $bundleId: UUID!, $itemId: UUID!) {",
  "removeBundleItem(idempotencyKey: $idempotencyKey, bundleId: $bundleId, itemId: $itemId)",
  "locale?: string,\n  idempotencyKey = crypto.randomUUID()",
  "itemId: string,\n  idempotencyKey = crypto.randomUUID()",
  "...(input.position === undefined ? {} : { position: input.position })",
  "...(input.bundleProductId === undefined",
  "...(input.description === undefined",
  "...(input.metadata === undefined ? {} : { metadata: input.metadata })",
  "bundleType: filter.bundleType",
]) {
  need(nextBundles, marker, "next bundles client");
}
forbid(nextBundles, "position: input.position ?? 0", "next bundles client");
forbid(nextBundles, "bundle_type: filter.bundleType", "next bundles client");

// ── Boundary decision and remediation evidence ───────────────────────────────────────────────
for (const marker of [
  "`ProductCatalogCommandPort`",
  "`ProductCatalogSchemaWritePort`",
  "create_relation_idempotent",
  "create_bundle_idempotent",
  "is not an allowed integration path",
  "extending this document",
  "scoped_caller_idempotency_key",
  "receipt-result encoding, a missing result, or completion failure awaits transaction rollback",
  "any early owner error",
  "retryable failures retain a reclaimable lease",
]) {
  need(boundaryDoc, marker, "write boundary doc");
}
need(registry, "verify-product-relations-bundles-idempotency.mjs", "registry");
need(audit, "PROD-IDEM-001", "audit remediation log");
need(audit, "PROD-PORT-001", "audit remediation log");

if (failures.length > 0) {
  console.error("[verify-product-relations-bundles-idempotency] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

console.log(
  "[verify-product-relations-bundles-idempotency] Product owner writes, relations, and bundles complete tenant-scoped owner receipts inside their write transaction, replay stored results, expose required idempotency keys on GraphQL and native admin transports, and keep the write boundary documented",
);
