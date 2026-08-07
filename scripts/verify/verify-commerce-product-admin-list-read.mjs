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
  console.error(`commerce product admin-list read guard failed: ${message}`);
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
  const end = source.indexOf(`pub async fn ${nextName}(`, start + 1);
  return source.slice(start, end < 0 ? source.length : end);
}

const ports = read("crates/rustok-product/src/ports.rs");
requireText(
  ports,
  "async fn list_admin_products(",
  "ProductCatalogReadPort must publish admin list reads",
);
requireText(
  ports,
  "pub struct AdminProductsRequest",
  "Product owner must publish the typed admin list request",
);
requireText(
  ports,
  "product.admin_list_unavailable",
  "ProductCatalogReadPort admin list must remain an optional fail-closed capability",
);
requireText(
  ports,
  "product admin listing is unavailable",
  "optional Product admin list capability must expose a stable unavailable message",
);
requireText(
  ports,
  "require_policy(PortCallPolicy::read())",
  "admin list owner call must preserve read deadline policy",
);

const queryTypes = read("crates/rustok-product/src/services/catalog/types.rs");
for (const required of [
  "pub raw_status: Option<String>",
  "pub vendor: Option<String>",
  "pub product_type: Option<String>",
  "pub empty_missing_title: bool",
]) {
  requireText(queryTypes, required, `owner admin list query must retain ${required}`);
}

const ownerQuery = read("crates/rustok-product/src/services/catalog/admin_queries.rs");
for (const required of [
  "Column::Status.eq(raw_status)",
  "Column::Vendor.eq(vendor)",
  "Column::ProductType.eq(product_type)",
  "if list_query.empty_missing_title",
]) {
  requireText(ownerQuery, required, `owner admin list implementation must contain ${required}`);
}

const adminProducts = read("crates/rustok-commerce/src/controllers/admin/products.rs");
const list = functionSlice(adminProducts, "list_products", "create_product");
for (const required of [
  ".product_catalog_read_port()",
  ".list_admin_products(",
  "rustok_product::AdminProductsRequest",
  "raw_status: params.status",
  "vendor: params.vendor",
  "product_type: params.product_type",
  "empty_missing_title: true",
  "StorefrontProductSortBy::CreatedAt",
  "StorefrontProductSortDirection::Desc",
  "fallback_locale: Some(tenant.default_locale.clone())",
  ".with_deadline(std::time::Duration::from_secs(2))",
  "unwrap_or_else(|| \"default\".to_string())",
]) {
  requireText(list, required, `mounted admin list must contain ${required}`);
}
for (const forbidden of [
  "super::super::products::list_products",
  "CatalogService::new",
  "product::Entity::find",
  "product_translation::Entity::find",
]) {
  forbidText(list, forbidden, `mounted admin list must not contain ${forbidden}`);
}

const sharedProducts = read("crates/rustok-commerce/src/controllers/products.rs");
requireText(
  sharedProducts,
  "pub async fn list_products(",
  "unmounted compatibility list source must remain explicit until removal evidence exists",
);

if (!process.exitCode) {
  console.log("commerce product admin-list read guard: source contract OK");
}
