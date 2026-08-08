#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "../..");
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), "utf8");

const lib = read("crates/rustok-product/admin/src/lib.rs");
const identity = read("crates/rustok-product/admin/src/lifecycle_retry_identity.rs");
const failures = [];

function requireText(source, text, label) {
  if (!source.includes(text)) failures.push(`${label}: missing ${text}`);
}

requireText(lib, "mod lifecycle_retry_identity;", "Product Admin FFA module registration");

for (const required of [
  "pub(crate) enum ProductAdminLifecycleOperation",
  "CreateProduct",
  "UpdateProduct",
  "ChangeStatus",
  "DeleteProduct",
  "pub(crate) struct ProductAdminLifecycleRetryIdentity<I>",
  "pending: Option<PendingLifecycleInvocation<I>>",
  "pub(crate) fn idempotency_key_for(",
  "pending.operation == operation",
  "&pending.intent == intent",
  '"product-admin:{}:{}"',
  "Uuid::new_v4()",
  "pub(crate) fn mark_succeeded(&mut self)",
  "self.pending = None",
  "explicit_retry_reuses_the_same_caller_key",
  "changed_intent_rotates_the_caller_key",
  "changed_operation_rotates_even_when_intent_matches",
  "successful_completion_releases_identity_for_a_later_equal_command",
  "generated_keys_fit_the_graphql_contract_limit",
  "assert!(key.len() <= 191)",
]) {
  requireText(identity, required, "Product Admin lifecycle retry identity");
}

if (failures.length) {
  console.error("Product Admin lifecycle retry identity source verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log("✔ Product Admin publishes a retry-stable lifecycle caller identity contract");
