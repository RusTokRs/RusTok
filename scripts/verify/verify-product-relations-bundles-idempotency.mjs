#!/usr/bin/env node
//
// PROD-IDEM-001 guard: Product relations and bundles are write-like owner commands, so every
// external entry point must carry a caller-owned idempotency key and complete a durable owner
// receipt inside the same transaction as the write.
//
// The guard pins the receipt protocol of both owner crates, the transaction cores the receipt-bound
// commands share with the plain port methods, the required GraphQL/native arguments, and the
// rejection of any parallel receipt ledger.

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
const relationsServicesModPath = "crates/modules/rustok-product-relations/src/services/mod.rs";
const relationsLibPath = "crates/modules/rustok-product-relations/src/lib.rs";
const relationsManifestPath = "crates/modules/rustok-product-relations/Cargo.toml";
const relationsAdapterPath =
  "crates/modules/rustok-product-relations/admin/src/transport/native_server_adapter.rs";
const bundlesReceiptsPath = "crates/modules/rustok-product-bundles/src/services/receipts.rs";
const bundlesServicePath = "crates/modules/rustok-product-bundles/src/services/bundle_service.rs";
const bundlesServicesModPath = "crates/modules/rustok-product-bundles/src/services/mod.rs";
const bundlesLibPath = "crates/modules/rustok-product-bundles/src/lib.rs";
const bundlesManifestPath = "crates/modules/rustok-product-bundles/Cargo.toml";
const bundlesAdapterPath =
  "crates/modules/rustok-product-bundles/admin/src/transport/native_server_adapter.rs";
const graphqlPath = "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs";
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
  relationsServicesModPath,
  relationsLibPath,
  relationsManifestPath,
  relationsAdapterPath,
  bundlesReceiptsPath,
  bundlesServicePath,
  bundlesServicesModPath,
  bundlesLibPath,
  bundlesManifestPath,
  bundlesAdapterPath,
  graphqlPath,
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
const relationsServicesMod = read(relationsServicesModPath);
const relationsLib = read(relationsLibPath);
const relationsManifest = read(relationsManifestPath);
const relationsAdapter = read(relationsAdapterPath);
const bundlesReceipts = read(bundlesReceiptsPath);
const bundlesService = read(bundlesServicePath);
const bundlesServicesMod = read(bundlesServicesModPath);
const bundlesLib = read(bundlesLibPath);
const bundlesManifest = read(bundlesManifestPath);
const bundlesAdapter = read(bundlesAdapterPath);
const graphql = read(graphqlPath);
const relationsGraphqlAdapter = read(relationsGraphqlAdapterPath);
const bundlesGraphqlAdapter = read(bundlesGraphqlAdapterPath);
const nextRelations = read(nextRelationsPath);
const nextBundles = read(nextBundlesPath);
const boundaryDoc = read(boundaryDocPath);
const registry = read(registryPath);
const audit = read(auditPath);

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

// ── Relations receipt protocol ───────────────────────────────────────────────────────────────
for (const marker of [
  'pub const PRODUCT_RELATION_OWNER_SLUG: &str = "product_relations";',
  'pub const CREATE_RELATION_OPERATION: &str = "create_relation";',
  "pub struct ProductRelationCommandContext<'a>",
  "idempotency::OwnerOperationScope::Tenant(context.tenant_id)",
  "idempotency::admit(",
  "idempotency::Admission::Run(lease) => lease,",
  "idempotency::Admission::Replay(value) => return decode_relation_receipt(value),",
  "idempotency::complete(&txn, lease, &relation).await?;",
  "idempotency::fail(self.database(), lease, &port_error).await",
  "create_relation_in_tx(&txn, context.tenant_id, input)",
  "pub fn relation_command_error(error: &ProductRelationError) -> PortError",
]) {
  need(relationsReceipts, marker, "relations receipts");
}
need(
  relationsService,
  "pub(crate) async fn create_relation_in_tx(",
  "relations service transaction core",
);
need(relationsService, "pub(crate) fn database(&self) -> &DatabaseConnection", "relations service");
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
  "idempotency::complete(&txn, lease, &bundle).await?;",
  "idempotency::complete(&txn, lease, &created_item).await?;",
  "idempotency::fail(service.database(), lease, &port_error).await",
  "create_bundle_in_tx(&txn, context.tenant_id, input)",
  "add_bundle_item_in_tx(&txn, context.tenant_id, bundle_id, item)",
  "pub fn bundle_command_error(error: &BundleError) -> PortError",
  '"product_bundle.slug_conflict"',
  '"product_bundle.item_not_found"',
]) {
  need(bundlesReceipts, marker, "bundles receipts");
}
for (const marker of [
  "pub(crate) async fn create_bundle_in_tx(",
  "pub(crate) async fn add_bundle_item_in_tx(",
  "pub(crate) fn database(&self) -> &DatabaseConnection",
]) {
  need(bundlesService, marker, "bundles service transaction core");
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

// ── Commerce GraphQL entry points ────────────────────────────────────────────────────────────
const relationMutation = slice(graphql, "async fn add_product_relation(", "async fn remove_product_relation(");
for (const marker of [
  "idempotency_key: String,",
  'validate_scoped_idempotency_key(&idempotency_key, "Product relation")?',
  '"commerce-graphql-relation"',
  "ProductRelationCommandContext::new(",
  ".create_relation_idempotent(",
  "relation_command_graphql_error(operation, error)",
]) {
  need(relationMutation, marker, "add_product_relation mutation");
}
forbid(relationMutation, "idempotency_key: Option<String>", "add_product_relation mutation");
forbid(relationMutation, ".create_relation(tenant_id", "add_product_relation mutation");

const bundleMutation = slice(graphql, "async fn create_bundle(", "async fn update_bundle(");
for (const marker of [
  "idempotency_key: String,",
  'validate_scoped_idempotency_key(&idempotency_key, "Product bundle")?',
  '"commerce-graphql-bundle"',
  "BundleCommandContext::new(",
  ".create_bundle_idempotent(",
  "bundle_command_graphql_error(operation, error)",
]) {
  need(bundleMutation, marker, "create_bundle mutation");
}

const bundleItemMutation = slice(graphql, "async fn add_bundle_item(", "async fn remove_bundle_item(");
for (const marker of [
  "idempotency_key: String,",
  "Some(bundle_id),",
  "BundleCommandContext::new(",
  ".add_bundle_item_idempotent(",
  "bundle_command_graphql_error(operation, error)",
]) {
  need(bundleItemMutation, marker, "add_bundle_item mutation");
}
for (const marker of ['"outbox.operation_receipt_conflict"', '"IDEMPOTENCY_KEY_CONFLICT"', "outbox.operation_receipt_in_progress"]) {
  need(graphql, marker, "catalog write command error vocabulary");
}
if (count(graphql, ".create_relation(") !== 0) {
  failures.push("catalog mutations: forbidden non-idempotent create_relation call");
}
if (count(graphql, ".create_bundle(") !== 0 || count(graphql, ".add_bundle_item(") !== 0) {
  failures.push("catalog mutations: forbidden non-idempotent bundle write call");
}

// ── Native admin transports use the caller key they already receive ──────────────────────────
for (const [source, label] of [
  [relationsAdapter, "relations native adapter"],
  [bundlesAdapter, "bundles native adapter"],
]) {
  forbid(source, "let _ = idempotency_key;", label);
}
for (const marker of [
  "create_relation_idempotent(",
  "ProductRelationCommandContext::new(",
  "relation_command_error_copy(&error)",
]) {
  need(relationsAdapter, marker, "relations native adapter");
}
for (const marker of [
  "create_bundle_idempotent(",
  "add_bundle_item_idempotent(",
  "BundleCommandContext::new(",
  "bundle_command_error_copy(&error)",
]) {
  need(bundlesAdapter, marker, "bundles native adapter");
}

// ── Remote GraphQL adapters carry the same contract ──────────────────────────────────────────
for (const marker of [
  "mutation ProductAdminAddRelation($idempotencyKey: String!, $input: AddProductRelationInput!) {",
  "addProductRelation(idempotencyKey: $idempotencyKey, input: $input) {",
  "idempotency_key: String,",
  'idempotency_key: String,',
  "#[serde(rename = \"idempotencyKey\")]",
  "idempotency_key,",
]) {
  need(relationsGraphqlAdapter, marker, "relations remote graphql adapter");
}
forbid(relationsGraphqlAdapter, "_idempotency_key", "relations remote graphql adapter");
for (const marker of [
  "mutation BundleAdminCreate($idempotencyKey: String!, $input: CreateBundleInputGql!) {",
  "createBundle(idempotencyKey: $idempotencyKey, input: $input) {",
  "mutation BundleAdminAddItem($idempotencyKey: String!, $bundleId: UUID!, $input: AddBundleItemInputGql!) {",
  "addBundleItem(idempotencyKey: $idempotencyKey, bundleId: $bundleId, input: $input) {",
]) {
  need(bundlesGraphqlAdapter, marker, "bundles remote graphql adapter");
}
forbid(bundlesGraphqlAdapter, "_idempotency_key", "bundles remote graphql adapter");

// ── FFA Next admin clients mint and send their own key ───────────────────────────────────────
need(
  nextRelations,
  "mutation ProductAdminAddRelation($idempotencyKey: String!, $input: AddProductRelationInput!) {",
  "next relations client",
);
need(nextRelations, "addProductRelation(idempotencyKey: $idempotencyKey, input: $input) {", "next relations client");
need(nextRelations, "const idempotencyKey = crypto.randomUUID();", "next relations client");
need(nextRelations, "{ idempotencyKey, input },", "next relations client");
for (const marker of [
  "mutation CreateBundle($idempotencyKey: String!, $input: CreateBundleInputGql!, $locale: String) {",
  "createBundle(idempotencyKey: $idempotencyKey, input: $input, locale: $locale) {",
  "mutation AddBundleItem($idempotencyKey: String!, $bundleId: UUID!, $item: BundleItemInputGql!) {",
  "addBundleItem(idempotencyKey: $idempotencyKey, bundleId: $bundleId, item: $item) {",
]) {
  need(nextBundles, marker, "next bundles client");
}
if ((nextBundles.match(/const idempotencyKey = crypto\.randomUUID\(\);/g) ?? []).length < 2) {
  failures.push("next bundles client: both bundle writes must mint their own idempotency key");
}

// ── Boundary decision and remediation evidence ───────────────────────────────────────────────
for (const marker of [
  "`ProductCatalogCommandPort`",
  "`ProductCatalogSchemaWritePort`",
  "create_relation_idempotent",
  "create_bundle_idempotent",
  "is not an allowed integration path",
  "requires extending this document",
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
  "[verify-product-relations-bundles-idempotency] relations and bundles write through tenant-scoped owner receipts completed inside the write transaction, replay their stored result, expose a required idempotency key on GraphQL and the native admin transports, and keep the write boundary documented",
);
