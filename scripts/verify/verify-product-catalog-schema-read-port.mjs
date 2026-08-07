#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "../..");

function read(relativePath) {
  return fs.readFileSync(path.join(root, relativePath), "utf8");
}

function fail(message) {
  console.error(`Product catalog schema read-port guard failed: ${message}`);
  process.exitCode = 1;
}

function requireText(source, text, message) {
  if (!source.includes(text)) fail(message);
}

const port = read("crates/rustok-product/src/catalog_schema_read_port.rs");
for (const required of [
  "pub trait ProductCatalogSchemaReadPort",
  "async fn list_attributes(",
  "async fn list_categories(",
  "async fn list_schemas(",
  "require_policy(PortCallPolicy::read())",
  "ProductCatalogSchemaService::list_attributes(",
  "ProductCatalogSchemaService::list_categories(",
  "ProductCatalogSchemaService::list_schemas(",
  "correlation_id = %context.correlation_id",
  '"product.database_unavailable"',
  '"product.validation"',
]) {
  requireText(port, required, `schema read port must contain ${required}`);
}

const runtime = read("crates/rustok-product/src/runtime.rs");
for (const required of [
  "schema_read_port: Option<Arc<dyn ProductCatalogSchemaReadPort>>",
  "schema_read_port: None",
  ".with_schema_read_port(Arc::new(ProductCatalogSchemaService::new(db, event_bus)))",
  "pub fn with_schema_read_port(",
  "pub fn schema_read_port(&self)",
]) {
  requireText(runtime, required, `Product read runtime must contain ${required}`);
}

const lib = read("crates/rustok-product/src/lib.rs");
for (const required of [
  "mod catalog_schema_read_port;",
  "pub use catalog_schema_read_port::ProductCatalogSchemaReadPort;",
]) {
  requireText(lib, required, `Product root must contain ${required}`);
}

const commerceQuery = read("crates/rustok-commerce/src/graphql/query.rs");
requireText(
  commerceQuery,
  "ProductCatalogSchemaService::new",
  "Commerce GraphQL schema-directory consumer cutover must remain explicit follow-up debt",
);

if (!process.exitCode) {
  console.log("Product catalog schema read-port guard: source contract OK");
}
