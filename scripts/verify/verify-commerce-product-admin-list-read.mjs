#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "../..");

function read(relativePath) {
  const filePath = path.join(root, relativePath);
  if (fs.existsSync(filePath)) return fs.readFileSync(filePath, "utf8");
  const dirRelative = relativePath.endsWith(".rs") ? relativePath.slice(0, -3) : relativePath;
  const dirPath = path.join(root, dirRelative);
  if (fs.existsSync(dirPath)) {
    return fs.readdirSync(dirPath)
      .filter((file) => file.endsWith(".rs"))
      .map((file) => fs.readFileSync(path.join(dirPath, file), "utf8"))
      .join("\n");
  }
  return fs.readFileSync(filePath, "utf8");
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

const ports = read("crates/modules/rustok-product/src/ports.rs");
for (const required of [
  "async fn list_admin_products(",
  "pub struct AdminProductsRequest",
  "pub raw_status: Option<String>",
  "pub vendor: Option<String>",
  "pub product_type: Option<String>",
  "pub empty_missing_title: bool",
  "product.admin_list_unavailable",
  "product admin listing is unavailable",
  "list_admin_products_with_compatibility_query(",
  "require_policy(PortCallPolicy::read())",
  "let page = page.max(1);",
]) {
  requireText(ports, required, `Product admin list port contract must contain ${required}`);
}

const queryTypes = read("crates/modules/rustok-product/src/services/catalog/types.rs");
const queryStart = queryTypes.indexOf("pub struct AdminProductListQuery {");
const queryEnd = queryTypes.indexOf("impl AdminProductListQuery", queryStart);
const querySlice = queryStart >= 0 && queryEnd > queryStart ? queryTypes.slice(queryStart, queryEnd) : queryTypes;
for (const forbidden of [
  "pub raw_status: Option<String>",
  "pub vendor: Option<String>",
  "pub product_type: Option<String>",
  "pub empty_missing_title: bool",
]) {
  forbidText(
    querySlice,
    forbidden,
    `existing AdminProductListQuery public shape must not gain compatibility field ${forbidden}`,
  );
}

const ownerQuery = read("crates/modules/rustok-product/src/services/catalog/admin_queries.rs");
for (const required of [
  "list_admin_products_with_compatibility_query(",
  "Column::Status.eq(raw_status)",
  "Column::Vendor.eq(vendor)",
  "Column::ProductType.eq(product_type)",
  "if empty_missing_title",
  "String::new()",
  "if legacy_shipping_profile_fallback",
  ".and_then(normalize_shipping_profile_slug)",
  "extract_shipping_profile_slug(&product.metadata)",
]) {
  requireText(ownerQuery, required, `owner admin list implementation must contain ${required}`);
}

const adminProducts = read("crates/modules/rustok-commerce/src/controllers/admin/products.rs");
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

const sharedProducts = read("crates/modules/rustok-commerce/src/controllers/products.rs");
requireText(
  sharedProducts,
  "pub async fn list_products(",
  "unmounted compatibility list source must remain explicit until removal evidence exists",
);

if (!process.exitCode) {
  console.log("commerce product admin-list read guard: source contract OK");
}
