#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");
const fail = (message) => {
  console.error(`Product schema-write consumer cutover guard failed: ${message}`);
  process.exit(1);
};
const requireText = (source, text, message) => {
  if (!source.includes(text)) fail(message ?? `missing ${JSON.stringify(text)}`);
};
const forbidText = (source, text, message) => {
  if (source.includes(text)) fail(message ?? `forbidden ${JSON.stringify(text)}`);
};

const server = read("crates/rustok-commerce/src/graphql/mutations/catalog.rs");
const activeTransport = read("crates/rustok-product/admin/src/catalog_transport_retry.rs");
const schemaTransport = read("crates/rustok-product/admin/src/transport/product_schema_graphql.rs");
const retryIdentity = read("crates/rustok-product/admin/src/schema_retry_identity.rs");
const adminLib = read("crates/rustok-product/admin/src/lib.rs");

forbidText(
  server,
  "ProductCatalogSchemaService",
  "mounted Commerce GraphQL must not construct the Product schema service directly",
);
requireText(server, "ProductCatalogSchemaWritePort", "mounted Commerce must depend on the typed Product schema-write port");
requireText(server, ".schema_write_port()", "mounted Commerce must resolve schema writes from the host-selected Product command runtime");
requireText(server, "product.schema_write_port_unavailable", "missing external schema-write capability must fail closed");
requireText(server, "Product schema mutation idempotency key is required", "omitted schema-write caller identity must be rejected before owner execution");
requireText(server, "with_idempotency_key(scoped_key)", "schema writes must reuse the scoped Product PortContext idempotency identity");
requireText(server, "with_deadline(PRODUCT_COMMAND_DEADLINE)", "schema writes must retain the Product command deadline");

const schemaResolvers = [
  ["create_product_attribute", "create_attribute"],
  ["create_product_attribute_option", "create_attribute_option"],
  ["create_catalog_category", "create_category"],
  ["create_product_attribute_schema", "create_schema"],
  ["create_product_attribute_schema_group", "create_schema_group"],
  ["create_catalog_category_attribute_group", "create_category_group"],
  ["set_catalog_category_schema_mode", "set_category_schema_mode"],
  ["bind_product_attribute_schema_attribute", "bind_schema_attribute"],
  ["bind_catalog_category_attribute", "bind_category_attribute"],
  ["save_product_attribute_values", "save_product_attribute_values"],
  ["clear_detached_product_attribute_values", "clear_detached_product_attribute_values"],
];

for (const [resolver, ownerMethod] of schemaResolvers) {
  const start = server.indexOf(`async fn ${resolver}(`);
  if (start < 0) fail(`missing schema resolver ${resolver}`);
  const next = server.indexOf("\n    async fn ", start + 1);
  const end = next < 0 ? server.length : next;
  const slice = server.slice(start, end);
  requireText(
    slice,
    "idempotency_key: Option<String>",
    `${resolver} must keep nullable SDL only as the explicit admission-ordering follow-up`,
  );
  requireText(slice, "product_schema_write_context(", `${resolver} must reject omitted caller identity before owner execution`);
  requireText(slice, `.${ownerMethod}(`, `${resolver} must call Product owner schema-write method ${ownerMethod}`);
  requireText(slice, "product_schema_write_port(ctx, &port_context)?", `${resolver} must resolve the host-composed schema-write capability`);
}

requireText(
  activeTransport,
  "pub(crate) use crate::product_schema_graphql::{",
  "active Product Admin transport must source schema writes from the retry-aware GraphQL module",
);
requireText(adminLib, "mod product_schema_graphql;", "Product Admin must mount the schema-write GraphQL transport");
requireText(adminLib, "mod schema_retry_identity;", "Product Admin must mount schema-write retry identity state");

const requiredVariableCount = (schemaTransport.match(/\$idempotencyKey: String!/g) ?? []).length;
if (requiredVariableCount !== schemaResolvers.length) {
  fail(`Product Admin must declare one required idempotency variable for each schema mutation (expected ${schemaResolvers.length}, found ${requiredVariableCount})`);
}
const forwardedKeyCount = (schemaTransport.match(/idempotencyKey: \$idempotencyKey/g) ?? []).length;
if (forwardedKeyCount !== schemaResolvers.length) {
  fail(`Product Admin must forward idempotencyKey on every schema mutation (expected ${schemaResolvers.length}, found ${forwardedKeyCount})`);
}
requireText(schemaTransport, "retained_caller_key", "Product Admin schema transport must retain caller identity across failed explicit retries");
requireText(schemaTransport, "mark_succeeded", "Product Admin schema transport must release caller identity only after success");
requireText(retryIdentity, "ProductAdminSchemaRetryIdentity", "Product Admin must own a typed schema retry identity capability");
requireText(retryIdentity, "pending.operation == operation", "schema retry identity must require the same operation");
requireText(retryIdentity, "&pending.intent == intent", "schema retry identity must require the exact same intent");
requireText(retryIdentity, "product-admin-schema:", "schema retry keys must use the Product Admin schema namespace");

console.log(
  "Product schema-write consumer cutover source guard passed: mounted Commerce uses the host-selected owner port, successful execution requires caller identity, Product Admin sends retained String! keys, and nullable server SDL remains an explicit admission-ordering follow-up.",
);
