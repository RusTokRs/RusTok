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

function forbidText(source, text, message) {
  if (source.includes(text)) fail(message);
}

function resolverSlice(source, name, nextName) {
  const start = source.indexOf(`async fn ${name}(`);
  if (start < 0) {
    fail(`missing resolver ${name}`);
    return "";
  }
  const end = nextName ? source.indexOf(`async fn ${nextName}(`, start + 1) : -1;
  return source.slice(start, end < 0 ? source.length : end);
}

const port = read("crates/rustok-product/src/catalog_schema_read_port.rs");
for (const required of [
  "pub trait ProductCatalogSchemaReadPort",
  "async fn list_attributes(",
  "async fn list_categories(",
  "async fn list_schemas(",
  "async fn read_effective_form(",
  "pub enum ProductEffectiveFormSubject",
  "pub struct ProductEffectiveFormRequest",
  "pub struct ProductEffectiveFormProjection",
  "pub struct ProductEffectiveFormAttributeProjection",
  '"product.effective_form_unavailable"',
  "product effective form is unavailable",
  "require_policy(PortCallPolicy::read())",
  "ProductCatalogSchemaService::list_attributes(",
  "ProductCatalogSchemaService::list_categories(",
  "ProductCatalogSchemaService::list_schemas(",
  "ProductCatalogSchemaService::load_effective_form_for_product(",
  "ProductCatalogSchemaService::load_effective_form_for_category(",
  "ProductCatalogSchemaService::load_effective_form_group_labels(",
  "ProductCatalogSchemaService::list_attribute_options(",
  '"product.attribute_definition_missing"',
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
  "pub use catalog_schema_read_port::{",
  "ProductCatalogSchemaReadPort",
  "ProductEffectiveFormAttributeProjection",
  "ProductEffectiveFormProjection",
  "ProductEffectiveFormRequest",
  "ProductEffectiveFormSubject",
]) {
  requireText(lib, required, `Product root must contain ${required}`);
}

const productCatalog = read("crates/rustok-commerce/src/graphql/product_catalog.rs");
requireText(
  productCatalog,
  "pub(crate) fn product_catalog_port_error(",
  "Product GraphQL port-error mapper must be reusable by schema directory resolvers",
);

const commerceQuery = read("crates/rustok-commerce/src/graphql/query.rs");
for (const required of [
  "fn product_schema_read_port_context(",
  ".with_deadline(std::time::Duration::from_secs(2))",
  "fn product_schema_read_port(",
  "product_catalog_read_runtime_for_current_graphql_scope(",
  ".schema_read_port()",
  '"product.schema_read_unavailable"',
]) {
  requireText(commerceQuery, required, `Commerce schema read boundary must contain ${required}`);
}

const resolvers = [
  ["product_attributes", "catalog_categories", ".list_attributes("],
  ["catalog_categories", "product_attribute_schemas", ".list_categories("],
  ["product_attribute_schemas", "product_effective_form", ".list_schemas("],
];
for (const [name, nextName, ownerCall] of resolvers) {
  const resolver = resolverSlice(commerceQuery, name, nextName);
  for (const required of [
    "let auth = require_commerce_permission(",
    "product_query_tenant(ctx, tenant_id)",
    "rustok_api::PortActor::user(auth.user_id.to_string())",
    "product_schema_read_port_context(",
    "product_schema_read_port(",
    ownerCall,
    "product_catalog_port_error(",
  ]) {
    requireText(resolver, required, `${name} must contain ${required}`);
  }
  forbidText(
    resolver,
    "ProductCatalogSchemaService::new",
    `${name} must not construct ProductCatalogSchemaService`,
  );
}

const effectiveForm = resolverSlice(
  commerceQuery,
  "product_effective_form",
  "product_attribute_values",
);
requireText(
  effectiveForm,
  "ProductCatalogSchemaService::new",
  "productEffectiveForm consumer cutover must remain explicit follow-up debt in this capability slice",
);
forbidText(
  effectiveForm,
  ".read_effective_form(",
  "productEffectiveForm must not be marked cut over by the capability-only source guard",
);

if (!process.exitCode) {
  console.log("Product catalog schema read-port guard: source contract OK");
}
