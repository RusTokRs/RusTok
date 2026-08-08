#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "../..");
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), "utf8");

const port = read("crates/rustok-product/src/catalog_schema_write_port.rs");
const runtime = read("crates/rustok-product/src/runtime.rs");
const lib = read("crates/rustok-product/src/lib.rs");
const commerceMutations = read("crates/rustok-commerce/src/graphql/mutations/catalog.rs");
const failures = [];

function requireText(source, text, label) {
  if (!source.includes(text)) failures.push(`${label}: missing ${text}`);
}

function forbidText(source, text, label) {
  if (source.includes(text)) failures.push(`${label}: forbidden ${text}`);
}

for (const required of [
  "pub trait ProductCatalogSchemaWritePort: Send + Sync",
  "async fn create_attribute(",
  "async fn create_attribute_option(",
  "async fn create_category(",
  "async fn create_schema(",
  "async fn create_schema_group(",
  "async fn create_category_group(",
  "async fn set_category_schema_mode(",
  "async fn bind_schema_attribute(",
  "async fn bind_category_attribute(",
  "async fn save_product_attribute_values(",
  "async fn clear_detached_product_attribute_values(",
  "impl ProductCatalogSchemaWritePort for ProductCatalogSchemaService",
  "require_policy(PortCallPolicy::write())",
  "Uuid::parse_str(context.tenant_id.as_str())",
  "Uuid::parse_str(context.actor.id.as_str())",
  '"product.schema_database_unavailable"',
  '"product.schema_validation"',
  '"product.schema_invariant_violation"',
]) {
  requireText(port, required, "Product schema write owner port");
}

for (const forbidden of [
  "PortError::unavailable(\n            \"product.schema_database_unavailable\",\n            error.to_string()",
  "PortError::validation(\n            \"product.schema_validation\",\n            error.to_string()",
]) {
  forbidText(port, forbidden, "Product schema write public error safety");
}

for (const required of [
  "ProductCatalogSchemaWritePort",
  "schema_write_port: Option<Arc<dyn ProductCatalogSchemaWritePort>>",
  "schema_write_port: None",
  ".with_schema_write_port(Arc::new(ProductCatalogSchemaService::new(db, event_bus)))",
  "pub fn with_schema_write_port(",
  "pub fn schema_write_port(&self) -> Option<Arc<dyn ProductCatalogSchemaWritePort>>",
]) {
  requireText(runtime, required, "Product command runtime schema-write composition");
}

for (const required of [
  "mod catalog_schema_write_port;",
  "pub use catalog_schema_write_port::ProductCatalogSchemaWritePort;",
]) {
  requireText(lib, required, "Product schema write public export");
}

// Publication is intentionally separate from the Commerce consumer cutover. Keep the
// remaining direct construction visible so the canonical task cannot be falsely closed.
requireText(
  commerceMutations,
  "ProductCatalogSchemaService::new",
  "Commerce schema-write consumer debt remains explicit",
);

if (failures.length) {
  console.error("Product catalog schema write port source verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log("✔ Product publishes a typed schema-write capability with write-context policy; Commerce consumer cutover remains explicit debt");
