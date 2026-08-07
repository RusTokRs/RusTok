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
  console.error(`commerce Product admin GraphQL read guard failed: ${message}`);
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

const source = read("crates/rustok-commerce/src/graphql/product_catalog.rs");
const admin = resolverSlice(source, "admin_product_catalog", null);
const storefront = resolverSlice(source, "storefront_product_catalog", "admin_product_catalog");

for (const required of [
  "let auth = require_commerce_permission(",
  "product_query_tenant(ctx, tenant_id)",
  "page == 0 || per_page == 0 || per_page > 100",
  "PortActor::user(auth.user_id.to_string())",
  ".with_deadline(std::time::Duration::from_secs(2))",
  "context.channel_slug.as_deref()",
  "product_catalog_read_runtime_for_current_graphql_scope(",
  ".read_port()",
  ".list_admin_products(",
  "rustok_product::AdminProductsRequest",
  "fallback_locale: Some(tenant.default_locale.clone())",
  "raw_status: None",
  "vendor: None",
  "product_type: None",
  "empty_missing_title: false",
  "admin_product_catalog_port_error(&port_context, error)",
]) {
  requireText(admin, required, `admin Product catalog resolver must contain ${required}`);
}

for (const forbidden of [
  "CatalogService::new",
  ".list_admin_products_with_query(",
  "product::Entity::find",
  "product_translation::Entity::find",
]) {
  forbidText(admin, forbidden, `admin Product catalog resolver must not contain ${forbidden}`);
}

for (const required of [
  '"PRODUCT_VALIDATION"',
  '"PRODUCT_NOT_FOUND"',
  '"PRODUCT_TEMPORARILY_UNAVAILABLE"',
  '"PRODUCT_OPERATION_FAILED"',
  'extensions.set("correlation_id", context.correlation_id.to_string())',
]) {
  requireText(source, required, `Product GraphQL PortError mapper must contain ${required}`);
}
for (const forbidden of [
  "Error::new(error.message)",
  'extensions.set("message", error.message)',
]) {
  forbidText(source, forbidden, `Product GraphQL owner errors must not expose ${forbidden}`);
}

requireText(
  storefront,
  "CatalogService::new",
  "storefront Product catalog must remain explicit follow-up debt in this slice",
);

const runtime = read("crates/rustok-commerce/src/graphql_runtime.rs");
for (const required of [
  "CURRENT_COMMERCE_PRODUCT_CATALOG_READ_RUNTIME",
  "product_catalog_read_runtime_for_current_graphql_scope(",
  "runtime_data.product_catalog_read_runtime()",
]) {
  requireText(runtime, required, `mounted GraphQL runtime must contain ${required}`);
}

if (!process.exitCode) {
  console.log("commerce Product admin GraphQL read guard: source contract OK");
}
