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
  const start = source.indexOf(`pub async fn ${name}(`);
  if (start < 0) {
    fail(`missing function ${name}`);
    return "";
  }
  const end = nextName ? source.indexOf(`pub async fn ${nextName}(`, start + 1) : source.length;
  return source.slice(start, end < 0 ? source.length : end);
}

const productLib = read("crates/rustok-product/src/lib.rs");
requireText(
  productLib,
  "pub use catalog_command_port::ProductCatalogCommandPort;",
  "rustok-product must publish ProductCatalogCommandPort",
);
requireText(
  productLib,
  "ProductCatalogCommandProfile, ProductCatalogCommandRuntime",
  "rustok-product must publish ProductCatalogCommandRuntime",
);

const productRuntime = read("crates/rustok-product/src/runtime.rs");
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

const commerceHttp = read("crates/rustok-commerce/src/controllers/mod.rs");
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

const adminProducts = read("crates/rustok-commerce/src/controllers/admin/products.rs");
forbidText(
  adminProducts,
  "CatalogService",
  "mounted admin Product CRUD must not construct or import CatalogService",
);
for (const method of ["create_product", "update_product"]) {
  const body = functionSlice(
    adminProducts,
    method,
    method === "create_product" ? "show_product" : "delete_product",
  );
  requireText(
    body,
    ".product_catalog_command_port()",
    `${method} must call the host-composed Product command port`,
  );
}

const sharedProducts = read("crates/rustok-commerce/src/controllers/products.rs");
for (const [method, next] of [
  ["delete_product", "publish_product"],
  ["publish_product", "unpublish_product"],
  ["unpublish_product", null],
]) {
  const body = functionSlice(sharedProducts, method, next);
  forbidText(
    body,
    "CatalogService::new",
    `${method} must not construct CatalogService`,
  );
  requireText(
    body,
    ".product_catalog_command_port()",
    `${method} must call the host-composed Product command port`,
  );
}

if (!process.exitCode) {
  console.log("commerce product command-port guard: source contract OK");
}
