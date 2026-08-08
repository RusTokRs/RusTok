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
  console.error(`commerce Product REST lifecycle cutover guard failed: ${message}`);
  process.exitCode = 1;
}

function requireText(source, text, message) {
  if (!source.includes(text)) fail(message);
}

function forbidText(source, text, message) {
  if (source.includes(text)) fail(message);
}

function functionSlice(source, name, nextName) {
  const start = source.indexOf(`pub async fn ${name}(`);
  if (start < 0) {
    fail(`missing function ${name}`);
    return "";
  }
  const end = nextName ? source.indexOf(`pub async fn ${nextName}(`, start + 1) : -1;
  return source.slice(start, end < 0 ? source.length : end);
}

const shared = read("crates/rustok-commerce/src/controllers/products.rs");
const mounted = read("crates/rustok-commerce/src/controllers/admin/products.rs");
const router = read("crates/rustok-commerce/src/controllers/admin/mod.rs");
const runtime = read("crates/rustok-commerce/src/controllers/mod.rs");
const ownerPort = read("crates/rustok-product/src/catalog_command_port.rs");

for (const required of [
  "async fn delete_product(",
  "async fn publish_product(",
  "async fn unpublish_product(",
  "PortCallPolicy::write()",
]) {
  requireText(ownerPort, required, `Product owner command port must contain ${required}`);
}

for (const required of [
  "product_catalog_command_runtime: rustok_product::ProductCatalogCommandRuntime",
  "fn product_catalog_command_port(",
  ".product_catalog_command_runtime.command_port()",
  ".shared_get::<rustok_product::ProductCatalogCommandRuntime>()",
]) {
  requireText(runtime, required, `Commerce HTTP runtime must contain ${required}`);
}

const lifecycleKeyStart = shared.indexOf("pub(crate) fn admin_product_lifecycle_idempotency_key(");
const lifecycleContextStart = shared.indexOf("pub(crate) fn admin_product_command_context(", lifecycleKeyStart);
const lifecycleKey = shared.slice(lifecycleKeyStart, lifecycleContextStart);
for (const required of [
  'headers.get("idempotency-key")',
  '"product_idempotency_key_required"',
  '"Idempotency-Key header is required"',
  "MAX_ADMIN_PRODUCT_LIFECYCLE_KEY_LENGTH",
  '"product_idempotency_key_invalid"',
  "digest.update(tenant_id.as_bytes())",
  "digest.update(actor_id.as_bytes())",
  "digest.update(product_id.as_bytes())",
  "digest.update(operation.as_bytes())",
  "digest.update(caller_key.as_bytes())",
]) {
  requireText(lifecycleKey, required, `lifecycle caller identity must contain ${required}`);
}

const deleteHandler = functionSlice(shared, "delete_product", "publish_product");
const publishHandler = functionSlice(shared, "publish_product", "unpublish_product");
const unpublishHandler = functionSlice(shared, "unpublish_product", null);

for (const [handler, operation, permission, portCall] of [
  [deleteHandler, "delete_product", "Permission::PRODUCTS_DELETE", ".delete_product(port_context.clone(), id)"],
  [publishHandler, "publish_product", "Permission::PRODUCTS_UPDATE", ".publish_product(port_context.clone(), id)"],
  [unpublishHandler, "unpublish_product", "Permission::PRODUCTS_UPDATE", ".unpublish_product(port_context.clone(), id)"],
]) {
  for (const required of [
    permission,
    "request_context: RequestContext",
    "headers: HeaderMap",
    "admin_product_lifecycle_idempotency_key(",
    `"${operation}"`,
    "admin_product_command_context(",
    ".product_catalog_command_port()",
    portCall,
    "map_admin_product_port_error(",
  ]) {
    requireText(handler, required, `${operation} handler must contain ${required}`);
  }
  for (const forbidden of [
    "CatalogService::new",
    `.operate_product(tenant.id, auth.user_id, id)`,
    `.delete_product(tenant.id, auth.user_id, id)`,
    `.publish_product(tenant.id, auth.user_id, id)`,
    `.unpublish_product(tenant.id, auth.user_id, id)`,
    "map_admin_product_error(",
  ]) {
    forbidText(handler, forbidden, `${operation} handler must not contain ${forbidden}`);
  }
}

for (const [name, nextName] of [
  ["delete_product", "publish_product"],
  ["publish_product", "unpublish_product"],
  ["unpublish_product", null],
]) {
  const handler = functionSlice(mounted, name, nextName);
  for (const required of [
    '("Idempotency-Key" = String, Header',
    "request_context: RequestContext",
    "headers: HeaderMap",
    `super::super::products::${name}(`,
    "request_context,",
    "headers,",
  ]) {
    requireText(handler, required, `mounted ${name} wrapper must contain ${required}`);
  }
}

for (const required of [
  ".delete(products::delete_product)",
  "axum::routing::post(products::publish_product)",
  "axum::routing::post(products::unpublish_product)",
]) {
  requireText(router, required, `mounted Product lifecycle route must contain ${required}`);
}

if (!process.exitCode) {
  console.log("commerce Product REST lifecycle cutover guard: source contract OK");
}
