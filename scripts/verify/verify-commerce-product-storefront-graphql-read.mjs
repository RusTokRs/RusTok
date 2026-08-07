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
  console.error(`commerce Product storefront GraphQL read guard failed: ${message}`);
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

const ports = read("crates/rustok-product/src/ports.rs");
for (const required of [
  "async fn list_filtered_published_products(",
  "pub struct FilteredPublishedProductsRequest",
  "pub query: StorefrontProductListQuery",
  "product.filtered_published_list_unavailable",
  "filtered product listing is unavailable",
  "LIST_FILTERED_PUBLISHED_PRODUCTS_OPERATION",
  "CatalogService::list_published_products_with_query(",
]) {
  requireText(ports, required, `filtered Product read capability must contain ${required}`);
}

const publishedStart = ports.indexOf("pub struct PublishedProductsRequest");
const filteredStart = ports.indexOf("pub struct FilteredPublishedProductsRequest", publishedStart);
const published = ports.slice(publishedStart, filteredStart);
for (const forbidden of ["search", "category_id", "sort_by", "attribute_filters", "query:"]) {
  forbidText(
    published,
    forbidden,
    `existing PublishedProductsRequest public shape must not gain filtered field ${forbidden}`,
  );
}

const source = read("crates/rustok-commerce/src/graphql/product_catalog.rs");
const storefront = resolverSlice(source, "storefront_product_catalog", "admin_product_catalog");
const admin = resolverSlice(source, "admin_product_catalog", null);

for (const required of [
  "StorefrontProductListQuery::try_new_with_attribute_filters(",
  ".with_pagination(page, per_page)",
  "PortActor::service(\"commerce-storefront-graphql\")",
  ".with_deadline(std::time::Duration::from_secs(2))",
  "public_channel_slug.as_deref()",
  "product_catalog_read_runtime_for_current_graphql_scope(",
  ".read_port()",
  ".list_filtered_published_products(",
  "rustok_product::FilteredPublishedProductsRequest",
  "fallback_locale: Some(tenant.default_locale.clone())",
  "public_channel_slug,",
  "query: list_query",
  "product_catalog_port_error(&port_context, error, \"storefront_product_catalog\")",
]) {
  requireText(storefront, required, `storefront Product catalog resolver must contain ${required}`);
}

for (const forbidden of [
  "CatalogService::new",
  ".list_published_products_with_query(",
  "product::Entity::find",
  "product_translation::Entity::find",
]) {
  forbidText(storefront, forbidden, `storefront Product catalog resolver must not contain ${forbidden}`);
}

for (const required of [
  ".list_admin_products(",
  "product_catalog_port_error(&port_context, error, \"admin_product_catalog\")",
]) {
  requireText(admin, required, `admin Product catalog cutover must remain intact: ${required}`);
}
forbidText(source, "CatalogService::new", "Product GraphQL catalog module must not construct CatalogService");

for (const required of [
  '"PRODUCT_VALIDATION"',
  '"PRODUCT_TEMPORARILY_UNAVAILABLE"',
  'extensions.set("correlation_id", context.correlation_id.to_string())',
]) {
  requireText(source, required, `shared Product GraphQL PortError mapper must contain ${required}`);
}
for (const forbidden of ["Error::new(error.message)", 'extensions.set("message", error.message)']) {
  forbidText(source, forbidden, `Product GraphQL owner errors must not expose ${forbidden}`);
}

if (!process.exitCode) {
  console.log("commerce Product storefront GraphQL read guard: source contract OK");
}
