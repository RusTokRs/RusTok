#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "../..");
const source = fs.readFileSync(
  path.join(root, "crates/rustok-commerce/src/controllers/admin/products.rs"),
  "utf8",
);

function fail(message) {
  console.error(`commerce product admin-detail read guard failed: ${message}`);
  process.exitCode = 1;
}

function showProductSlice() {
  const start = source.indexOf("pub async fn show_product(");
  const end = source.indexOf("pub async fn update_product(", start + 1);
  if (start < 0 || end < 0) {
    fail("could not isolate mounted show_product handler");
    return "";
  }
  return source.slice(start, end);
}

const show = showProductSlice();
for (const required of [
  ".product_catalog_read_port()",
  ".read_product_projection(",
  "rustok_product::ProductProjectionRequest",
  "fallback_locale: Some(tenant.default_locale.clone())",
  ".with_deadline(std::time::Duration::from_secs(2))",
  "map_admin_product_port_error(",
]) {
  if (!show.includes(required)) {
    fail(`show_product must contain ${required}`);
  }
}

for (const forbidden of [
  "CatalogService::new",
  "super::super::products::show_product",
  "get_product_with_locale_fallback",
]) {
  if (show.includes(forbidden)) {
    fail(`show_product must not contain ${forbidden}`);
  }
}

const listStart = source.indexOf("pub async fn list_products(");
const createStart = source.indexOf("pub async fn create_product(", listStart + 1);
const list = source.slice(listStart, createStart);
if (!list.includes("super::super::products::list_products")) {
  fail("admin list must remain explicit follow-up debt in this slice");
}

if (!process.exitCode) {
  console.log("commerce product admin-detail read guard: source contract OK");
}
