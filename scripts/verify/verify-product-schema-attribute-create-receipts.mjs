#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");
const failures = [];
const requireText = (source, text, label) => {
  if (!source.includes(text)) failures.push(`${label}: missing ${text}`);
};
const functionSlice = (source, name, nextName) => {
  const start = source.indexOf(`async fn ${name}(`);
  if (start < 0) {
    failures.push(`missing function ${name}`);
    return "";
  }
  const end = nextName ? source.indexOf(`async fn ${nextName}(`, start + 1) : -1;
  return source.slice(start, end < 0 ? source.length : end);
};
const requireRecordedBeforeCommit = (source, expected, label) => {
  const recorded = source.indexOf(expected);
  const commit = source.indexOf("txn.commit().await?");
  if (recorded < 0 || commit < 0 || recorded > commit) {
    failures.push(`${label}: receipt result must be recorded before owner transaction commit`);
  }
};

const port = read("crates/rustok-product/src/catalog_schema_write_port.rs");
const transaction = read("crates/rustok-product/src/services/write_transaction.rs");
const attributes = read("crates/rustok-product/src/services/catalog_schema_service/attributes.rs");
const categories = read("crates/rustok-product/src/services/catalog_schema_service/categories.rs");
const schemas = read("crates/rustok-product/src/services/catalog_schema_service/schemas.rs");
const values = read("crates/rustok-product/src/services/catalog_schema_service/values.rs");
const effectiveForms = read("crates/rustok-product/src/services/catalog_schema_service/effective_forms.rs");
const recheck = read("crates/rustok-commerce/docs/product-schema-write-recheck-2026-08-08.md");
const implStart = port.indexOf("impl ProductCatalogSchemaWritePort for ProductCatalogSchemaService");
if (implStart < 0) failures.push("missing ProductCatalogSchemaWritePort implementation");
const portImpl = implStart < 0 ? "" : port.slice(implStart);

for (const required of [
  "rustok_outbox::idempotency",
  "admit_schema_operation(",
  "with_product_operation_receipt(",
  "decode_schema_receipt(",
  "finish_receipted_schema_write(",
  "fail_schema_operation_receipt",
  '"product.schema_idempotency_conflict"',
  '"product.schema_receipt_unavailable"',
]) {
  requireText(port, required, "Product schema receipt port");
}

for (const [name, nextName] of [
  ["create_attribute", "create_attribute_option"],
  ["create_attribute_option", "create_category"],
  ["create_category", "create_schema"],
  ["create_schema", "create_schema_group"],
  ["create_schema_group", "create_category_group"],
  ["create_category_group", "set_category_schema_mode"],
  ["set_category_schema_mode", "bind_schema_attribute"],
  ["bind_schema_attribute", "bind_category_attribute"],
  ["bind_category_attribute", "save_product_attribute_values"],
  ["save_product_attribute_values", "clear_detached_product_attribute_values"],
  ["clear_detached_product_attribute_values", "admit_schema_operation"],
]) {
  const slice = functionSlice(portImpl, name, nextName);
  for (const required of [
    "admit_schema_operation(",
    "idempotency::Admission::Replay(value)",
    "with_product_operation_receipt(",
    "finish_receipted_schema_write(",
  ]) {
    requireText(slice, required, `${name} durable receipt`);
  }
}

for (const required of [
  "tokio::task_local!",
  "struct ProductOperationReceipt",
  "Arc<Mutex<Option<Value>>>",
  "PRODUCT_OPERATION_RECEIPT.try_with(Clone::clone).ok()",
  "record_product_operation_result",
  "product owner receipt result was not recorded before commit",
  "idempotency::complete(&self.transaction, receipt.lease, &response_json)",
  "self.transaction.commit().await?",
  "current_product_operation_id()",
]) {
  requireText(transaction, required, "Product write transaction receipt fence");
}
const completion = transaction.indexOf("idempotency::complete(&self.transaction");
const commit = transaction.indexOf("self.transaction.commit().await?");
if (completion < 0 || commit < 0 || completion > commit) {
  failures.push("Product receipt completion must occur before owner transaction commit");
}

for (const [source, label, expected] of [
  [attributes, "attribute creates", 2],
  [categories, "category creates", 2],
  [schemas, "schema creates", 2],
]) {
  const stableIds = (source.match(/current_product_operation_id\(\)\.unwrap_or_else\(generate_id\)/g) ?? []).length;
  if (stableIds !== expected) {
    failures.push(`${label} must derive exactly ${expected} stable receipt resource IDs (found ${stableIds})`);
  }
  const recordedResults = (source.match(/record_product_operation_result\(&result\)\?/g) ?? []).length;
  if (recordedResults !== expected) {
    failures.push(`${label} must record exactly ${expected} actual receipt results before commit (found ${recordedResults})`);
  }
}

for (const required of [
  'const PRODUCT_SCHEMA_RECEIPT_OWNER: &str = "product"',
  "idempotency::admit(",
  "idempotency::fail(",
]) {
  requireText(attributes, required, "Product schema receipt owner helpers");
}

const categoryCreate = functionSlice(categories, "create_category", "list_categories");
for (const required of [
  "let path = parent",
  "let result = CatalogCategoryRecord",
  "path,",
  "record_product_operation_result(&result)?",
]) {
  requireText(categoryCreate, required, "category create actual-result receipt");
}

const categoryMode = functionSlice(categories, "set_category_schema_mode", "bind_category_attribute");
const categoryBinding = functionSlice(categories, "bind_category_attribute", null);
const schemaBinding = functionSlice(schemas, "bind_schema_attribute", null);
for (const [slice, label] of [
  [categoryMode, "category schema mode update"],
  [categoryBinding, "category attribute binding update"],
  [schemaBinding, "schema attribute binding update"],
]) {
  requireRecordedBeforeCommit(slice, "record_product_operation_result(&())?", label);
}

for (const required of [
  "pub(super) async fn load_product_attribute_values_in<C>",
  "C: ConnectionTrait",
  "Self::load_effective_form_for_product_in(",
  ".all(conn)",
]) {
  requireText(values, required, "transaction-local Product attribute-value projection");
}
for (const required of [
  "pub(super) async fn load_effective_form_for_product_in<C>",
  "async fn load_effective_form_for_category_in<C>",
  "async fn load_category_schema_map<C>",
  "async fn load_attribute_schema_map<C>",
  "C: ConnectionTrait",
]) {
  requireText(effectiveForms, required, "connection-neutral Product effective-form projection");
}

const saveValues = functionSlice(values, "save_product_attribute_values", "clear_detached_product_attribute_values");
for (const required of [
  "let result = Self::load_product_attribute_values_in(&txn, tenant_id, product_id, locale)",
  "record_product_operation_result(&result)?",
  "txn.commit().await?",
  "Ok(result)",
]) {
  requireText(saveValues, required, "attribute-value save exact receipt result");
}
requireRecordedBeforeCommit(
  saveValues,
  "record_product_operation_result(&result)?",
  "attribute-value save exact projection",
);
const saveProjection = saveValues.indexOf("Self::load_product_attribute_values_in(&txn");
const saveCommit = saveValues.indexOf("txn.commit().await?");
if (saveProjection < 0 || saveCommit < 0 || saveProjection > saveCommit) {
  failures.push("attribute-value save projection must be loaded from the owner transaction before commit");
}

const clearValues = functionSlice(values, "clear_detached_product_attribute_values", null);
for (const required of [
  "let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?",
  "if !target_attribute_ids.is_empty()",
  "let result = Self::load_product_attribute_values_in(&txn, tenant_id, product_id, locale)",
  "record_product_operation_result(&result)?",
  "txn.commit().await?",
  "Ok(result)",
]) {
  requireText(clearValues, required, "detached attribute-value clear exact receipt result");
}
requireRecordedBeforeCommit(
  clearValues,
  "record_product_operation_result(&result)?",
  "detached attribute-value clear exact projection",
);
const clearTxn = clearValues.indexOf("let txn = ProductWriteTransaction::begin");
const clearEmptyGuard = clearValues.indexOf("if !target_attribute_ids.is_empty()");
const clearProjection = clearValues.indexOf("Self::load_product_attribute_values_in(&txn");
const clearCommit = clearValues.indexOf("txn.commit().await?");
if (
  clearTxn < 0 ||
  clearEmptyGuard < 0 ||
  clearProjection < 0 ||
  clearCommit < 0 ||
  clearTxn > clearEmptyGuard ||
  clearEmptyGuard > clearProjection ||
  clearProjection > clearCommit
) {
  failures.push("detached clear must complete both mutation and empty-target paths through one receipt-capable owner transaction");
}

for (const required of [
  "all eleven mounted Product schema writes",
  "exact completed projection",
  "empty-target detached clear",
  "source-complete durable owner replay",
]) {
  requireText(recheck, required, "Product schema write recheck must record completed receipt coverage");
}

if (failures.length) {
  console.error("Product schema receipt source verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log("✔ all eleven mounted Product schema writes use durable owner receipts; attribute-value results are captured from the owner transaction before commit");
