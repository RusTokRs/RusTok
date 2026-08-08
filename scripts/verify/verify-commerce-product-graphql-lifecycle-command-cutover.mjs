#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "../..");
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), "utf8");

const mutations = read("crates/rustok-commerce/src/graphql/mutations/catalog.rs");
const graphqlRuntime = read("crates/rustok-commerce/src/graphql_runtime.rs");
const ownerPort = read("crates/rustok-product/src/catalog_command_port.rs");
const hostComposition = read("apps/server/src/services/commerce_provider_runtime.rs");
const catalogFixture = read("crates/rustok-commerce/tests/graphql_runtime_parity_test/catalog.rs");
const shippingFixture = read("crates/rustok-commerce/tests/graphql_runtime_parity_test/shipping.rs");
const failures = [];

function fail(message) {
  failures.push(message);
}

function requireText(source, text, label) {
  if (!source.includes(text)) fail(`${label}: missing ${text}`);
}

function forbidText(source, text, label) {
  if (source.includes(text)) fail(`${label}: forbidden ${text}`);
}

function functionSlice(source, name, nextName) {
  const start = source.indexOf(`async fn ${name}(`);
  if (start < 0) {
    fail(`missing function ${name}`);
    return "";
  }
  const end = nextName ? source.indexOf(`async fn ${nextName}(`, start + 1) : -1;
  return source.slice(start, end < 0 ? source.length : end);
}

for (const required of [
  "pub trait ProductCatalogCommandPort",
  "async fn create_product(",
  "async fn update_product(",
  "async fn publish_product(",
  "async fn delete_product(",
  "PortCallPolicy::write()",
  '"product.duplicate_handle"',
  '"product.duplicate_sku"',
  '"product.no_variants"',
  '"product.lifecycle_conflict"',
]) {
  requireText(ownerPort, required, "Product owner command port");
}

for (const required of [
  "ProductCatalogCommandRuntime",
  ".shared_get::<rustok_product::ProductCatalogCommandRuntime>()",
  "ProductCatalogCommandRuntime::in_process(",
  "host.with_shared_value(runtime)",
]) {
  requireText(hostComposition, required, "host Product command composition");
}

for (const required of [
  "ProductCatalogCommandRuntime",
  "CURRENT_COMMERCE_PRODUCT_CATALOG_COMMAND_RUNTIME",
  "runtime_data.product_catalog_command_runtime()",
  "product_catalog_command_runtime_for_current_graphql_scope(",
  "ProductCatalogCommandRuntime::in_process(db, event_bus)",
  "product_catalog_command_runtime: ProductCatalogCommandRuntime",
  "pub fn product_catalog_command_runtime(&self) -> ProductCatalogCommandRuntime",
  ".shared_get::<ProductCatalogCommandRuntime>()",
  "commerce GraphQL requires ProductCatalogCommandRuntime in host composition",
]) {
  requireText(graphqlRuntime, required, "scoped GraphQL Product command runtime");
}

for (const required of [
  "ProductCatalogCommandRuntime",
  "product_catalog_command_runtime_for_current_graphql_scope(",
  "MAX_PRODUCT_GRAPHQL_IDEMPOTENCY_KEY_LENGTH: usize = 191",
  "PRODUCT_COMMAND_DEADLINE: Duration = Duration::from_secs(2)",
  "idempotency_key: String",
  "Product mutation idempotency key must not be empty",
  "BAD_USER_INPUT",
  "let caller_key = idempotency_key.trim();",
  "digest.update(tenant_id.as_bytes())",
  "digest.update(user_id.as_bytes())",
  "digest.update(operation.as_bytes())",
  "digest.update(product_id.as_bytes())",
  "digest.update(caller_key.as_bytes())",
  "PortActor::user(user_id.to_string())",
  ".with_idempotency_key(scoped_key)",
  ".with_deadline(PRODUCT_COMMAND_DEADLINE)",
  "context = context.with_claim(permission.to_string())",
  "context = context.with_channel(channel)",
]) {
  requireText(mutations, required, "GraphQL Product command context");
}

for (const forbidden of [
  "Product mutation idempotency key is required",
  "one-request compatibility identity",
  'format!("compatibility-{}", Uuid::new_v4())',
  "Product GraphQL lifecycle caller omitted idempotency key",
]) {
  forbidText(mutations, forbidden, "GraphQL Product lifecycle mandatory caller identity");
}

forbidText(
  mutations,
  "HostRuntimeContext",
  "Product lifecycle resolver must use Commerce GraphQL scoped runtime instead of direct host lookup",
);

for (const required of [
  '"DUPLICATE_HANDLE"',
  '"DUPLICATE_SKU"',
  '"NO_VARIANTS"',
  '"CANNOT_DELETE_PUBLISHED"',
  '"PRODUCT_VALIDATION"',
  '"PRODUCT_NOT_FOUND"',
  '"PRODUCT_TEMPORARILY_UNAVAILABLE"',
  '"PRODUCT_OPERATION_FAILED"',
  'extensions.set("retryable", error.retryable)',
  'extensions.set("correlation_id", context.correlation_id.clone())',
]) {
  requireText(mutations, required, "GraphQL Product lifecycle error parity");
}

const create = functionSlice(mutations, "create_product", "update_product");
const update = functionSlice(mutations, "update_product", "publish_product");
const publish = functionSlice(mutations, "publish_product", "delete_product");
const remove = functionSlice(mutations, "delete_product", "create_product_attribute");

for (const [slice, operation, permission, portCall, productIdentity] of [
  [create, "create_product", "Permission::PRODUCTS_CREATE", ".create_product(port_context.clone(), domain_input)", "None"],
  [update, "update_product", "Permission::PRODUCTS_UPDATE", ".update_product(port_context.clone(), id, domain_input)", "Some(id)"],
  [publish, "publish_product", "Permission::PRODUCTS_UPDATE", ".publish_product(port_context.clone(), id)", "Some(id)"],
  [remove, "delete_product", "Permission::PRODUCTS_DELETE", ".delete_product(port_context.clone(), id)", "Some(id)"],
]) {
  for (const required of [
    "idempotency_key: String",
    permission,
    "product_mutation_actor(ctx)?",
    "product_command_context(",
    productIdentity,
    `"${operation}"`,
    "product_command_runtime(ctx)?",
    ".command_port()",
    portCall,
    "product_command_port_error(",
  ]) {
    requireText(slice, required, `${operation} GraphQL mutation`);
  }
  forbidText(slice, "idempotency_key: Option<String>", `${operation} GraphQL mutation`);
  for (const forbidden of [
    "CatalogService::new",
    `.create_product(tenant_id, user_id`,
    `.update_product(tenant_id, user_id`,
    `.publish_product(tenant_id, user_id`,
    `.delete_product(tenant_id, user_id`,
    "product_catalog_port_error(",
  ]) {
    forbidText(slice, forbidden, `${operation} GraphQL mutation`);
  }
}

for (const required of [
  "validate_product_shipping_profile_input(",
  "input.shipping_profile_slug.as_deref()",
  "convert_create_product_input(input)?",
]) {
  requireText(create, required, "create Product compatibility semantics");
}
for (const required of [
  "validate_product_shipping_profile_input(",
  "input.shipping_profile_slug.as_deref()",
  "metadata: None",
  "status: input.status.map(Into::into)",
]) {
  requireText(update, required, "update Product compatibility semantics");
}

for (const required of [
  'createProduct(idempotencyKey: "foreign-actor-create"',
  'createProduct(idempotencyKey: "foreign-actor-create-all"',
  'updateProduct(idempotencyKey: "foreign-actor-update-all"',
  'publishProduct(idempotencyKey: "foreign-actor-publish-all"',
  'deleteProduct(idempotencyKey: "foreign-actor-delete-all"',
]) {
  requireText(catalogFixture, required, "Product lifecycle foreign-actor fixture");
}
requireText(
  shippingFixture,
  'idempotencyKey: "unknown-shipping-profile-product"',
  "Product lifecycle shipping-profile fixture",
);

forbidText(
  mutations,
  "ProductCatalogSchemaService::new",
  "mounted Product schema writes must not regress to direct owner-service construction",
);
for (const required of [
  "ProductCatalogSchemaWritePort",
  ".schema_write_port()",
  "product.schema_write_port_unavailable",
]) {
  requireText(mutations, required, "Product schema-write owner boundary");
}
forbidText(
  mutations,
  "use rustok_product::{CatalogService,",
  "GraphQL lifecycle must not import CatalogService",
);

if (failures.length) {
  console.error("Commerce Product GraphQL lifecycle command cutover verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log("✔ Product GraphQL lifecycle SDL requires explicit caller idempotency; mounted schema writes retain the typed owner boundary with no direct Product schema service construction");
