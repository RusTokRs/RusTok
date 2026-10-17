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
const outboxIdempotencyPath = "crates/modules/rustok-outbox/src/idempotency.rs";
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
  outboxIdempotencyPath,
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
const outboxIdempotency = read(outboxIdempotencyPath);
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
for (const marker of [
  "pub fn scoped_caller_idempotency_key(",
  "tenant_id: Uuid,",
  "actor_id: Uuid,",
  "caller_key.trim().as_bytes()",
]) {
  need(outboxIdempotency, marker, "shared caller-key scoping primitive");
}
for (const [source, namespace, label] of [
  [relationsLib, 'RELATION_CALLER_IDEMPOTENCY_NAMESPACE: &str = "commerce-graphql-relation"', "relations receipt key owner"],
  [bundlesLib, 'BUNDLE_CALLER_IDEMPOTENCY_NAMESPACE: &str = "commerce-graphql-bundle"', "bundles receipt key owner"],
]) {
  need(source, namespace, label);
  need(source, "pub fn scoped_caller_idempotency_key(", label);
  need(source, "rustok_outbox::idempotency::scoped_caller_idempotency_key(", label);
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
  "idempotency::fail(",
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
const relationReceiptCommands = [
  [
    "create_relation_idempotent",
    "pub async fn create_relation_idempotent(",
    "pub async fn update_relation_idempotent(",
    "admit_relation_operation(self, context, CREATE_RELATION_OPERATION, &request)",
    "create_relation_in_tx(&txn, context.tenant_id, input)",
    "idempotency::complete(&txn, lease, &relation).await?;",
  ],
  [
    "update_relation_idempotent",
    "pub async fn update_relation_idempotent(",
    "pub async fn delete_relation_idempotent(",
    "admit_relation_operation(self, context, UPDATE_RELATION_OPERATION, &request)",
    "update_relation_in_tx(&txn, context.tenant_id, relation_id, input)",
    "idempotency::complete(&txn, lease, &relation).await?;",
  ],
  [
    "delete_relation_idempotent",
    "pub async fn delete_relation_idempotent(",
    "pub async fn reorder_relations_idempotent(",
    "admit_relation_operation(self, context, DELETE_RELATION_OPERATION, &request)",
    "delete_relation_in_tx(&txn, context.tenant_id, relation_id)",
    "idempotency::complete(&txn, lease, &()).await?;",
  ],
  [
    "reorder_relations_idempotent",
    "pub async fn reorder_relations_idempotent(",
    "async fn admit_relation_operation",
    "admit_relation_operation(self, context, REORDER_RELATIONS_OPERATION, &request)",
    "reorder_relations_in_tx(&txn, context.tenant_id, input)",
    "idempotency::complete(&txn, lease, &relations).await?;",
  ],
];
for (const [name, start, end, admission, write, completion] of relationReceiptCommands) {
  const command = slice(relationsReceipts, start, end);
  for (const marker of [admission, write, completion, "finish_failed_relation_operation("]) {
    need(command, marker, `${name} receipt command`);
  }
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
  "fn map_unique_constraint_conflict(error: DbErr) -> BundleError",
  "Some(sea_orm::SqlErr::UniqueConstraintViolation(_))",
  "BundleError::ConcurrentWriteConflict",
  ".map_err(map_unique_constraint_conflict)?",
]) {
  need(bundlesService, marker, "bundles service transaction core");
}
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
    "idempotency::complete(&txn, lease, &bundle).await?;",
  ],
  [
    "add_bundle_item_idempotent",
    "pub async fn add_bundle_item_idempotent(",
    "pub async fn update_bundle_idempotent(",
    "admit_bundle_operation(self, context, ADD_BUNDLE_ITEM_OPERATION, &request)",
    "add_bundle_item_in_tx(&txn, context.tenant_id, bundle_id, item)",
    "idempotency::complete(&txn, lease, &created_item).await?;",
  ],
  [
    "update_bundle_idempotent",
    "pub async fn update_bundle_idempotent(",
    "pub async fn delete_bundle_idempotent(",
    "admit_bundle_operation(self, context, UPDATE_BUNDLE_OPERATION, &request)",
    "update_bundle_in_tx(&txn, context.tenant_id, bundle_id, input)",
    "idempotency::complete(&txn, lease, &bundle).await?;",
  ],
  [
    "delete_bundle_idempotent",
    "pub async fn delete_bundle_idempotent(",
    "pub async fn remove_bundle_item_idempotent(",
    "admit_bundle_operation(self, context, DELETE_BUNDLE_OPERATION, &request)",
    "delete_bundle_in_tx(&txn, context.tenant_id, bundle_id)",
    "idempotency::complete(&txn, lease, &()).await?;",
  ],
  [
    "remove_bundle_item_idempotent",
    "pub async fn remove_bundle_item_idempotent(",
    "async fn admit_bundle_operation",
    "admit_bundle_operation(",
    "remove_bundle_item_in_tx(&txn, context.tenant_id, bundle_id, item_id)",
    "idempotency::complete(&txn, lease, &()).await?;",
  ],
];
for (const [name, start, end, admission, write, completion] of bundleReceiptCommands) {
  const command = slice(bundlesReceipts, start, end);
  for (const marker of [admission, write, completion, "finish_failed_bundle_operation("]) {
    need(command, marker, `${name} receipt command`);
  }
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
]) {
  need(relationsAdapter, marker, "relations native adapter");
}
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
]) {
  need(bundlesAdapter, marker, "bundles native adapter");
}
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
  "mutation BundleAdminCreate($idempotencyKey: String!, $input: CreateBundleInputGql!) {",
  "createBundle(idempotencyKey: $idempotencyKey, input: $input) {",
  "mutation BundleAdminUpdate($idempotencyKey: String!, $id: UUID!, $input: UpdateBundleInputGql!) {",
  "updateBundle(idempotencyKey: $idempotencyKey, id: $id, input: $input) {",
  "mutation BundleAdminDelete($idempotencyKey: String!, $id: UUID!) {",
  "deleteBundle(idempotencyKey: $idempotencyKey, id: $id)",
  "mutation BundleAdminAddItem($idempotencyKey: String!, $bundleId: UUID!, $input: AddBundleItemInputGql!) {",
  "addBundleItem(idempotencyKey: $idempotencyKey, bundleId: $bundleId, input: $input) {",
  "mutation BundleAdminRemoveItem($idempotencyKey: String!, $bundleId: UUID!, $itemId: UUID!) {",
  "removeBundleItem(idempotencyKey: $idempotencyKey, bundleId: $bundleId, itemId: $itemId)",
  "idempotency_key: String,",
  "#[serde(rename = \"idempotencyKey\")]",
]) {
  need(bundlesGraphqlAdapter, marker, "bundles remote graphql adapter");
}
forbid(bundlesGraphqlAdapter, "_idempotency_key", "bundles remote graphql adapter");

// ── FFA Next admin clients mint and send a key for every write ────────────────────────────────
for (const marker of [
  "mutation ProductAdminAddRelation($idempotencyKey: String!, $input: AddProductRelationInput!) {",
  "addProductRelation(idempotencyKey: $idempotencyKey, input: $input) {",
  "mutation ProductAdminRemoveRelation($idempotencyKey: String!, $id: UUID!) {",
  "removeProductRelation(idempotencyKey: $idempotencyKey, id: $id)",
  "mutation ProductAdminReorderRelations($idempotencyKey: String!, $productId: UUID!, $relationType: GqlRelationType!, $orderedRelationIds: [UUID!]!) {",
  "reorderProductRelations(idempotencyKey: $idempotencyKey, productId: $productId, relationType: $relationType, orderedRelationIds: $orderedRelationIds) {",
]) {
  need(nextRelations, marker, "next relations client");
}
if ((nextRelations.match(/const idempotencyKey = crypto\.randomUUID\(\);/g) ?? []).length < 3) {
  failures.push("next relations client: every relation write must mint its own idempotency key");
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
]) {
  need(nextBundles, marker, "next bundles client");
}
if ((nextBundles.match(/const idempotencyKey = crypto\.randomUUID\(\);/g) ?? []).length < 5) {
  failures.push("next bundles client: every bundle write must mint its own idempotency key");
}

// ── Boundary decision and remediation evidence ───────────────────────────────────────────────
for (const marker of [
  "`ProductCatalogCommandPort`",
  "`ProductCatalogSchemaWritePort`",
  "create_relation_idempotent",
  "create_bundle_idempotent",
  "is not an allowed integration path",
  "extending this document",
  "scoped_caller_idempotency_key",
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
