#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

const facadePath = "crates/rustok-profiles/src/mutations.rs";
const profilesLibPath = "crates/rustok-profiles/src/lib.rs";
const graphqlPath = "crates/rustok-profiles/src/graphql/mutation.rs";
const cliPath = "crates/rustok-profiles/cli/src/lib.rs";
const legacyGateContractPath =
  "crates/rustok-forum/contracts/forum-search-profile-service-mutation-boundary.json";
const contractPath =
  "crates/rustok-forum/contracts/forum-search-profile-event-aware-mutation-api.json";
const notePath = "crates/rustok-forum/docs/forum-23a9-profile-event-aware-mutation-api.md";

const eventAwareMethods = [
  "upsert_profile_with_event",
  "update_profile_handle_with_event",
  "update_profile_content_with_event",
  "update_profile_locale_with_event",
  "update_profile_visibility_with_event",
  "update_profile_media_with_event",
  "backfill_profile_with_event",
];

function read(relativePath) {
  const target = path.join(root, relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(target, "utf8");
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function rejectMarker(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

function countOccurrences(source, marker) {
  let count = 0;
  let offset = 0;
  while (true) {
    const index = source.indexOf(marker, offset);
    if (index < 0) return count;
    count += 1;
    offset = index + marker.length;
  }
}

const facade = read(facadePath);
const profilesLib = read(profilesLibPath);
const graphql = read(graphqlPath);
const cli = read(cliPath);
const note = read(notePath);
let contract = null;
let legacyGateContract = null;
try {
  contract = JSON.parse(read(contractPath));
} catch (error) {
  failures.push(`${contractPath}: invalid JSON: ${error.message}`);
}
try {
  legacyGateContract = JSON.parse(read(legacyGateContractPath));
} catch (error) {
  failures.push(`${legacyGateContractPath}: invalid JSON: ${error.message}`);
}

for (const marker of [
  "pub struct ProfileMutationService<'a>",
  "db: &'a DatabaseConnection",
  "event_bus: &'a TransactionalEventBus",
  "pub fn new(db: &'a DatabaseConnection, event_bus: &'a TransactionalEventBus)",
  "Self { db, event_bus }",
  "commits the owner write only after",
]) {
  requireMarker(facade, marker, facadePath);
}

for (const method of eventAwareMethods) {
  requireMarker(facade, `pub async fn ${method}(`, facadePath);
  if (countOccurrences(facade, `${method}(`) < 2) {
    failures.push(`${facadePath}: ${method} must define the facade method and delegate to owner helper`);
  }
}
for (const marker of [
  "self.db",
  "self.event_bus",
  "upsert_write::{backfill_profile_with_event, upsert_profile_with_event}",
  "content_write::update_profile_content_with_event",
  "handle_write::update_profile_handle_with_event",
  "locale_write::update_profile_locale_with_event",
  "media_write::update_profile_media_with_event",
  "visibility_write::update_profile_visibility_with_event",
]) {
  requireMarker(facade, marker, facadePath);
}

requireMarker(profilesLib, "pub mod mutations;", profilesLibPath);
requireMarker(profilesLib, "pub use mutations::ProfileMutationService;", profilesLibPath);
requireMarker(profilesLib, "pub use upsert_write::backfill_profile_with_event;", profilesLibPath);

for (const marker of [
  "ProfileMutationService",
  "ProfileMutationService::new(db, event_bus)",
  "mutations.upsert_profile_with_event(",
  "mutations.update_profile_handle_with_event(",
  "mutations.update_profile_content_with_event(",
  "mutations.update_profile_locale_with_event(",
  "mutations.update_profile_visibility_with_event(",
  "mutations.update_profile_media_with_event(",
  "validate_profile_media_references(",
]) {
  requireMarker(graphql, marker, graphqlPath);
}
for (const marker of [
  "content_write::update_profile_content_with_event",
  "handle_write::update_profile_handle_with_event",
  "locale_write::update_profile_locale_with_event",
  "media_write::update_profile_media_with_event",
  "upsert_write::upsert_profile_with_event",
  "visibility_write::update_profile_visibility_with_event",
]) {
  rejectMarker(graphql, marker, graphqlPath);
}

for (const marker of [
  "ProfileMutationService",
  "ProfileMutationService::new(&db, &event_bus)",
  "profile_mutations",
  ".backfill_profile_with_event(",
  "let event_published = result.created;",
]) {
  requireMarker(cli, marker, cliPath);
}
rejectMarker(cli, "backfill_profile_with_event,", cliPath);
rejectMarker(cli, "let result = backfill_profile_with_event(", cliPath);

for (const marker of [
  "FORUM-23A9",
  "ProfileMutationService",
  "DatabaseConnection",
  "TransactionalEventBus",
  "GraphQL self-service",
  "CLI backfill",
  "Legacy boundary",
  "does not claim",
  "Not run by the implementation agent",
]) {
  requireMarker(note, marker, notePath);
}

if (legacyGateContract?.task !== "FORUM-23A8") {
  failures.push(`${legacyGateContractPath}: expected FORUM-23A8 source gate`);
}

if (contract) {
  if (contract.task !== "FORUM-23A9") failures.push(`${contractPath}: unexpected task`);
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${contractPath}: unexpected status`);
  }
  if (contract.mutation_facade !== facadePath) {
    failures.push(`${contractPath}: unexpected mutation facade`);
  }
  if (contract.profiles_public_api !== profilesLibPath) {
    failures.push(`${contractPath}: unexpected Profiles public API path`);
  }
  if (contract.graphql_runtime !== graphqlPath) {
    failures.push(`${contractPath}: unexpected GraphQL runtime path`);
  }
  if (contract.cli_backfill_runtime !== cliPath) {
    failures.push(`${contractPath}: unexpected CLI runtime path`);
  }
  if (contract.legacy_source_gate_contract !== legacyGateContractPath) {
    failures.push(`${contractPath}: unexpected legacy source gate contract`);
  }
  if (JSON.stringify(contract.public_event_aware_methods) !== JSON.stringify(eventAwareMethods)) {
    failures.push(`${contractPath}: public event-aware method inventory drift`);
  }

  for (const key of [
    "facade_is_publicly_exported",
    "facade_constructor_requires_database_connection",
    "facade_constructor_requires_transactional_event_bus",
    "all_facade_mutations_are_event_named",
    "facade_delegates_to_profiles_owned_atomic_helpers",
    "graphql_self_service_uses_facade",
    "graphql_self_service_actor_is_preserved",
    "graphql_media_validation_precedes_facade_write",
    "cli_backfill_uses_facade",
    "cli_backfill_system_actor_semantics_are_preserved",
    "legacy_repository_production_calls_remain_source_gated",
    "compatible_free_backfill_helper_remains_event_aware",
  ]) {
    if (contract.mutation_api_boundary?.[key] !== true) {
      failures.push(`${contractPath}: mutation API boundary ${key} drift`);
    }
  }

  for (const key of [
    "legacy_profile_service_signatures_removed",
    "public_backfill_helper_removed",
    "graphql_schema_changed",
    "forum_rest_changed",
    "search_query_api_changed",
    "search_document_schema_changed",
    "database_migration_added",
    "dependency_added",
    "cargo_lock_changed",
  ]) {
    if (contract.compatibility?.[key] !== false) {
      failures.push(`${contractPath}: compatibility ${key} must remain false`);
    }
  }

  for (const key of [
    "legacy_profile_service_mutations_are_compile_time_private",
    "external_downstream_crates_cannot_call_legacy_methods",
    "all_profiles_owner_write_surfaces_are_event_intrinsic",
    "runtime_verification_was_executed",
    "account_deletion_redaction_is_complete",
    "general_cross_producer_owner_revision_ordering_is_complete",
  ]) {
    if (contract.non_claims?.[key] !== true) {
      failures.push(`${contractPath}: non-claim ${key} drift`);
    }
  }

  if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
    failures.push(`${contractPath}: execution status drift`);
  }
  if (contract.downstream_task !== "FORUM-23A10") {
    failures.push(`${contractPath}: unexpected downstream task`);
  }
}

if (failures.length > 0) {
  console.error("Profiles event-aware mutation API verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Profiles event-aware mutation API verification passed.");
