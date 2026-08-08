#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "../..");
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), "utf8");

const lib = read("crates/rustok-product/admin/src/lib.rs");
const facade = read("crates/rustok-product/admin/src/catalog_transport_retry.rs");
const adapter = read("crates/rustok-product/admin/src/transport/product_lifecycle_graphql.rs");
const identity = read("crates/rustok-product/admin/src/lifecycle_retry_identity.rs");
const commerce = read("crates/rustok-commerce/src/graphql/mutations/catalog.rs");
const failures = [];

function requireText(source, text, label) {
  if (!source.includes(text)) failures.push(`${label}: missing ${text}`);
}

function forbidText(source, text, label) {
  if (source.includes(text)) failures.push(`${label}: forbidden ${text}`);
}

for (const required of [
  '#[path = "catalog_transport.rs"]\nmod legacy_transport;',
  '#[path = "catalog_transport_retry.rs"]\nmod transport;',
  '#[path = "transport/product_lifecycle_graphql.rs"]\nmod product_lifecycle_graphql;',
]) {
  requireText(lib, required, "Product Admin active transport wiring");
}

for (const required of [
  "ProductAdminLifecycleRetryIdentity",
  "ProductAdminLifecycleOperation::CreateProduct",
  "ProductAdminLifecycleOperation::UpdateProduct",
  "ProductAdminLifecycleOperation::ChangeStatus",
  "ProductAdminLifecycleOperation::DeleteProduct",
  "OnceLock<Mutex<HashMap<String, RetryIdentity>>>",
  ".idempotency_key_for(operation, &intent)",
  "if result.is_ok()",
  "mark_lifecycle_succeeded(&slot);",
  "crate::product_lifecycle_graphql::create_product(",
  "crate::product_lifecycle_graphql::update_product(",
  "crate::product_lifecycle_graphql::change_product_status(",
  "crate::product_lifecycle_graphql::delete_product(",
]) {
  requireText(facade, required, "Product Admin lifecycle retry consumer facade");
}

requireText(identity, "pub(crate) fn mark_succeeded(&mut self)", "published retry identity contract");
requireText(identity, "pending.operation == operation", "published retry identity operation match");
requireText(identity, "&pending.intent == intent", "published retry identity exact-intent match");

for (const required of [
  "$idempotencyKey: String!",
  "createProduct(idempotencyKey: $idempotencyKey, input: $input)",
  "updateProduct(idempotencyKey: $idempotencyKey, id: $id, input: $input)",
  "deleteProduct(idempotencyKey: $idempotencyKey, id: $id)",
  '#[serde(rename = "idempotencyKey")]\n    idempotency_key: String',
  "PRODUCT_ADMIN_MUTATION_GRAPHQL_BOUNDARY",
  "PRODUCT_ADMIN_HTTP_PUBLIC_MESSAGE",
  "PRODUCT_ADMIN_GRAPHQL_PUBLIC_MESSAGE",
]) {
  requireText(adapter, required, "Product Admin retry-aware lifecycle GraphQL adapter");
}

forbidText(adapter, "compatibility-", "Product Admin explicit retry caller");
forbidText(adapter, "Option<String>\n    idempotency_key", "Product Admin explicit retry caller");

// This consumer slice deliberately stops before the server schema contract becomes required.
// Keeping the next debt visible prevents source inspection from over-claiming completion.
requireText(
  commerce,
  "idempotency_key: Option<String>",
  "mounted GraphQL mandatory-idempotency follow-up remains explicit",
);
requireText(
  commerce,
  "using one-request compatibility identity",
  "mounted GraphQL compatibility path remains explicit",
);

if (failures.length) {
  console.error("Product Admin lifecycle retry consumer source verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log("✔ Product Admin lifecycle callers retain explicit GraphQL retry identity; mandatory server schema remains follow-up debt");
