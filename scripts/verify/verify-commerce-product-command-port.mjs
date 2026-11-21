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
  console.error(`commerce product command-port guard failed: ${message}`);
  process.exitCode = 1;
}

function requireText(source, text, message) {
  if (!source.includes(text)) fail(message);
}

function forbidText(source, text, message) {
  if (source.includes(text)) fail(message);
}

function functionSlice(source, name, nextName) {
  const fnStart = source.indexOf(`pub async fn ${name}(`);
  if (fnStart < 0) {
    fail(`missing function ${name}`);
    return "";
  }
  const attrStart = source.lastIndexOf("#[utoipa::path(", fnStart);
  const start = attrStart < 0 ? fnStart : attrStart;
  const end = nextName ? source.indexOf(`pub async fn ${nextName}(`, start + 1) : source.length;
  return source.slice(start, end < 0 ? source.length : end);
}

const productLib = read("crates/modules/rustok-product/src/lib.rs");
requireText(
  productLib,
  "pub use catalog_command_port::{\n    ProductCatalogCommandPort,",
  "rustok-product must publish ProductCatalogCommandPort",
);
requireText(
  productLib,
  "ProductCatalogCommandProfile, ProductCatalogCommandRuntime",
  "rustok-product must publish ProductCatalogCommandRuntime",
);

const commandPort = read("crates/modules/rustok-product/src/catalog_command_port.rs");
requireText(
  commandPort,
  "catalog_scope(context, operation, PortCallPolicy::write())",
  "Product command port must enforce deadline and idempotency write semantics",
);

const productRuntime = read("crates/modules/rustok-product/src/runtime.rs");
requireText(
  productRuntime,
  "pub struct ProductCatalogCommandRuntime",
  "product command runtime must be owner-provided",
);
requireText(
  productRuntime,
  "pub fn external(command_port: Arc<dyn ProductCatalogCommandPort>)",
  "product command runtime must support host-selected external adapters",
);

const serverRuntime = read("apps/server/src/services/commerce_provider_runtime.rs");
requireText(
  serverRuntime,
  "shared_get::<rustok_product::ProductCatalogCommandRuntime>()",
  "server must preserve or compose ProductCatalogCommandRuntime",
);
requireText(
  serverRuntime,
  "rustok_product::ProductCatalogCommandRuntime::in_process",
  "server must provide the explicit embedded Product command adapter",
);

const commerceHttp = read("crates/modules/rustok-commerce/src/controllers/mod.rs");
requireText(
  commerceHttp,
  "product_catalog_command_runtime: rustok_product::ProductCatalogCommandRuntime",
  "Commerce HTTP runtime must store the host-composed Product command runtime",
);
requireText(
  commerceHttp,
  "Commerce HTTP routes require ProductCatalogCommandRuntime in HostRuntimeContext",
  "Commerce HTTP mount must fail closed when Product command runtime is missing",
);

const adminProducts = read("crates/modules/rustok-commerce/src/controllers/admin/products.rs");
forbidText(
  adminProducts,
  "CatalogService",
  "mounted admin Product create/update must not construct or import CatalogService",
);
for (const method of ["create_product", "update_product"]) {
  const body = functionSlice(
    adminProducts,
    method,
    method === "create_product" ? "show_product" : "delete_product",
  );
  requireText(
    body,
    "headers: HeaderMap,",
    `${method} must accept the caller-owned idempotency header`,
  );
  requireText(
    body,
    "admin_product_command_idempotency_key(&headers)?;",
    `${method} must validate the caller-owned idempotency key`,
  );
  requireText(
    body,
    "Idempotency-Key",
    `${method} must publish the caller-owned idempotency contract`,
  );
  requireText(
    body,
    ".product_catalog_command_port()",
    `${method} must call the host-composed Product command port`,
  );
}

const sharedProducts = read("crates/modules/rustok-commerce/src/controllers/products.rs");
requireText(
  sharedProducts,
  'fn admin_product_command_idempotency_key(',
  "shared Product command identity helper must require caller-owned identity",
);
requireText(
  sharedProducts,
  '.get("Idempotency-Key")',
  "shared Product command identity helper must read Idempotency-Key",
);
requireText(
  sharedProducts,
  'format!("commerce-admin-product:{operation}:{resource_id}")',
  "Product command correlation must be resource/operation scoped",
);
requireText(
  sharedProducts,
  ".with_idempotency_key(idempotency_key)",
  "mounted Product create/update command context must carry an idempotency key",
);
if (sharedProducts.includes("CatalogService::new(")) {
  fail("unmounted Product read/lifecycle direct construction must stay removed after the 2026-11-17 audit");
}

if (!process.exitCode) {
  console.log("commerce product command-port guard: source contract OK");
}

const syntheticIdentity = sharedProducts.includes('fn admin_product_command_idempotency_key<T: Serialize>(');
if (syntheticIdentity) {
  fail('synthetic payload-derived Product command identity helper remains');
}

const callerIdentityUses = (adminProducts.match(/admin_product_command_idempotency_key\(&headers\)\?/g) || []).length;
if (callerIdentityUses !== 2) {
  fail('expected two caller-owned Product create/update identity uses, found ' + callerIdentityUses);
}