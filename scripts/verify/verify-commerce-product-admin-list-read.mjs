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
]) {
  requireText(ports, required, `Product admin list port contract must contain ${required}`);
}

const queryTypes = read("crates/rustok-product/src/services/catalog/types.rs");
for (const forbidden of [
  "pub raw_status: Option<String>",
  "pub vendor: Option<String>",
  "pub product_type: Option<String>",
  "pub empty_missing_title: bool",
]) {
  forbidText(
    queryTypes,
    forbidden,
    `existing AdminProductListQuery public shape must not gain compatibility field ${forbidden}`,
  );
}

const ownerQuery = read("crates/rustok-product/src/services/catalog/admin_queries.rs");
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
