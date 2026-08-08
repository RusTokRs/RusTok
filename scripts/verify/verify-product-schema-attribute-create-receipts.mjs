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
const forbidText = (source, text, label) => {
  if (source.includes(text)) failures.push(`${label}: forbidden ${text}`);
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

const port = read("crates/rustok-product/src/catalog_schema_write_port.rs");
const transaction = read("crates/rustok-product/src/services/write_transaction.rs");
const attributes = read("crates/rustok-product/src/services/catalog_schema_service/attributes.rs");
const plan = read("crates/rustok-commerce/docs/implementation-plan.md");
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
  requireText(port, required, "Product schema create receipt port");
}

for (const [name, nextName] of [
  ["create_attribute", "create_attribute_option"],
  ["create_attribute_option", "create_category"],
]) {
  const slice = functionSlice(portImpl, name, nextName);
  for (const required of [
    "admit_schema_operation(",
    "idempotency::Admission::Replay(value)",
    "lease.operation_id",
    "with_product_operation_receipt(",
    "finish_receipted_schema_write(",
  ]) {
    requireText(slice, required, `${name} durable receipt`);
  }
}

for (const [name, nextName] of [
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
  forbidText(slice, "admit_schema_operation(", `${name} must remain explicit receipt follow-up debt in this slice`);
}

for (const required of [
  "tokio::task_local!",
  "struct ProductOperationReceipt",
  "PRODUCT_OPERATION_RECEIPT.try_with(Clone::clone).ok()",
  "idempotency::complete(&self.transaction, receipt.lease, &receipt.response_json)",
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

const stableIdCount = (attributes.match(/current_product_operation_id\(\)\.unwrap_or_else\(generate_id\)/g) ?? []).length;
if (stableIdCount !== 2) {
  failures.push(`attribute and option creates must derive exactly two stable receipt resource IDs (found ${stableIdCount})`);
}
for (const required of [
  'const PRODUCT_SCHEMA_RECEIPT_OWNER: &str = "product"',
  "idempotency::admit(",
  "idempotency::fail(",
]) {
  requireText(attributes, required, "Product schema receipt owner helpers");
}

requireText(
  plan,
  "receipts cover attribute and attribute-option creates",
  "canonical Product schema-write debt must record this partial durable slice",
);
requireText(
  plan,
  "category/schema/group\n  creates and update-style schema writes still need explicit owner replay semantics",
  "canonical Product schema-write debt must remain open",
);
requireText(
  recheck,
  "attribute and attribute-option creates",
  "Product schema write recheck must describe the durable create slice",
);
requireText(
  recheck,
  "category/schema/group creates and update-style schema writes remain open",
  "Product schema write recheck must preserve remaining debt",
);

if (failures.length) {
  console.error("Product schema attribute-create receipt source verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log("✔ Product attribute and attribute-option creates bind durable owner receipts atomically with Product writes; remaining schema replay semantics stay explicit debt");
