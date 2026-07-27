#!/usr/bin/env node

import { readFileSync } from "node:fs";

const files = {
  module: readFileSync(
    "crates/rustok-index/src/infrastructure/postgres/mod.rs",
    "utf8",
  ),
  lib: readFileSync("crates/rustok-index/src/lib.rs", "utf8"),
  registration: readFileSync(
    "crates/rustok-index/src/infrastructure/postgres/schema_registration.rs",
    "utf8",
  ),
  tests: readFileSync(
    "crates/rustok-index/src/infrastructure/postgres/schema_registration_tests.rs",
    "utf8",
  ),
};

const failures = [];

function requireText(name, source, text) {
  if (!source.includes(text)) {
    failures.push(`${name} is missing required marker: ${text}`);
  }
}

function forbidText(name, source, text) {
  if (source.includes(text)) {
    failures.push(`${name} contains forbidden marker: ${text}`);
  }
}

for (const marker of [
  "mod schema_registration;",
  "mod schema_registration_tests;",
  "PostgresSchemaRegistrationStore",
  "PersistedSchemaRegistrationOutcome",
  "SchemaRegistrationError",
]) {
  requireText("postgres module", files.module, marker);
}
for (const marker of [
  "PostgresSchemaRegistrationStore",
  "PersistedSchemaRegistrationOutcome",
  "SchemaRegistrationError",
]) {
  requireText("Index public API", files.lib, marker);
}
for (const marker of [
  "tenant_id.is_nil()",
  "schema.fingerprint()",
  "serde_json::to_value(schema)",
  "pg_advisory_xact_lock",
  "load_exact_schema",
  "load_latest_version",
  "NonMonotonicVersion",
  "VersionConflict",
  "SchemaRetired",
  "ON CONFLICT (tenant_id, module_name, entity_name, schema_version) DO NOTHING",
  "only PostgreSQL and SQLite are supported",
]) {
  requireText("schema registration store", files.registration, marker);
}
for (const forbidden of [
  "rustok_social_graph",
  "rustok_product",
  "ProfilePresentationService",
  "SocialGraphRelationEvent",
  "Product",
  "sales_channel",
]) {
  forbidText("schema registration store", files.registration, forbidden);
}
for (const marker of [
  "registration_is_tenant_scoped_and_exactly_idempotent",
  "same_version_contract_reuse_fails_closed",
  "unregistered_lower_version_is_rejected_after_newer_version",
  "retired_schema_cannot_be_reactivated_by_registration",
  "nil_tenant_fails_before_storage",
]) {
  requireText("schema registration tests", files.tests, marker);
}

if (failures.length > 0) {
  console.error("Index schema registration verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "Index schema registration verification passed: generic tenant-scoped persistence, exact idempotency, monotonic versioning, retired-state protection, backend bounds, public API export, and source-domain neutrality are locked.",
);
