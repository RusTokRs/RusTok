#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

const servicesPath = "crates/rustok-profiles/src/services.rs";
const facadePath = "crates/rustok-profiles/src/mutations.rs";
const profilesReadmePath = "crates/rustok-profiles/README.md";
const graphqlPath = "crates/rustok-profiles/src/graphql/mutation.rs";
const cliPath = "crates/rustok-profiles/cli/src/lib.rs";
const sourceGateContractPath =
  "crates/rustok-forum/contracts/forum-search-profile-service-mutation-boundary.json";
const eventAwareContractPath =
  "crates/rustok-forum/contracts/forum-search-profile-event-aware-mutation-api.json";
const contractPath =
  "crates/rustok-forum/contracts/forum-search-profile-legacy-mutation-deprecation.json";
const notePath =
  "crates/rustok-forum/docs/forum-23a10-profile-legacy-mutation-deprecation.md";

const deprecatedMethods = {
  upsert_profile: {
    replacement: "ProfileMutationService::upsert_profile_with_event",
    note:
      "use ProfileMutationService::upsert_profile_with_event so owner writes and durable ProfileUpdated publication remain atomic",
  },
  update_profile_handle: {
    replacement: "ProfileMutationService::update_profile_handle_with_event",
    note:
      "use ProfileMutationService::update_profile_handle_with_event so owner writes and durable ProfileUpdated publication remain atomic",
  },
  update_profile_content: {
    replacement: "ProfileMutationService::update_profile_content_with_event",
    note:
      "use ProfileMutationService::update_profile_content_with_event so owner writes and durable ProfileUpdated publication remain atomic",
  },
  update_profile_locale: {
    replacement: "ProfileMutationService::update_profile_locale_with_event",
    note:
      "use ProfileMutationService::update_profile_locale_with_event so owner writes and durable ProfileUpdated publication remain atomic",
  },
  update_profile_visibility: {
    replacement: "ProfileMutationService::update_profile_visibility_with_event",
    note:
      "use ProfileMutationService::update_profile_visibility_with_event so owner writes and durable ProfileUpdated publication remain atomic",
  },
  update_profile_media: {
    replacement: "ProfileMutationService::update_profile_media_with_event",
    note:
      "use ProfileMutationService::update_profile_media_with_event so owner writes and durable ProfileUpdated publication remain atomic",
  },
  backfill_profile: {
    replacement: "ProfileMutationService::backfill_profile_with_event",
    note:
      "use ProfileMutationService::backfill_profile_with_event so profile creation and durable ProfileUpdated publication remain atomic",
  },
};

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

function parseJson(relativePath) {
  try {
    return JSON.parse(read(relativePath));
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return null;
  }
}

const services = read(servicesPath);
const facade = read(facadePath);
const profilesReadme = read(profilesReadmePath);
const graphql = read(graphqlPath);
const cli = read(cliPath);
const note = read(notePath);
const sourceGateContract = parseJson(sourceGateContractPath);
const eventAwareContract = parseJson(eventAwareContractPath);
const contract = parseJson(contractPath);

if (countOccurrences(services, "#[deprecated(") !== Object.keys(deprecatedMethods).length) {
  failures.push(`${servicesPath}: expected exactly seven deprecated method attributes`);
}

for (const [method, policy] of Object.entries(deprecatedMethods)) {
  const expected = `#[deprecated(\n        note = "${policy.note}"\n    )]\n    pub async fn ${method}(`;
  requireMarker(services, expected, servicesPath);
  requireMarker(facade, `pub async fn ${method}_with_event(`, facadePath);
}

requireMarker(
  services,
  "#[allow(clippy::too_many_arguments, deprecated)]\n    #[deprecated(\n        note = \"use ProfileMutationService::backfill_profile_with_event",
  servicesPath,
);

for (const safeMethod of [
  "new",
  "normalize_handle",
  "normalize_display_name",
  "normalize_locale",
  "locale_candidates",
  "plan_backfill_profile",
  "get_profile",
  "get_profile_by_handle",
  "get_profile_summary",
  "get_profile_summaries",
]) {
  rejectMarker(
    services,
    `#[deprecated(\n        note = "use ProfileMutationService::${safeMethod}`,
    servicesPath,
  );
}

for (const marker of [
  "ProfileMutationService::new(db, event_bus)",
  "mutations.upsert_profile_with_event(",
  "mutations.update_profile_handle_with_event(",
  "mutations.update_profile_content_with_event(",
  "mutations.update_profile_locale_with_event(",
  "mutations.update_profile_visibility_with_event(",
  "mutations.update_profile_media_with_event(",
]) {
  requireMarker(graphql, marker, graphqlPath);
}
for (const marker of [
  "ProfileMutationService::new(&db, &event_bus)",
  "profile_mutations",
  ".backfill_profile_with_event(",
]) {
  requireMarker(cli, marker, cliPath);
}

for (const marker of [
  "older mutation methods on `ProfileService` only as deprecated compatibility shims",
  "New callers must use the corresponding event-aware `ProfileMutationService` methods",
  "ProfileService` for reads, normalization, planning, and deprecated mutation compatibility only",
  "ProfileMutationService` for production profile writes",
]) {
  requireMarker(profilesReadme, marker, profilesReadmePath);
}

for (const marker of [
  "FORUM-23A10",
  "Rust `#[deprecated]` diagnostics",
  "compiler deprecation",
  "FORUM-23A8",
  "FORUM-23A9",
  "does not claim",
  "Not run by the implementation agent",
]) {
  requireMarker(note, marker, notePath);
}

if (sourceGateContract?.task !== "FORUM-23A8") {
  failures.push(`${sourceGateContractPath}: expected FORUM-23A8 source gate`);
}
if (sourceGateContract?.source_boundary?.repository_production_call_sites_are_forbidden !== true) {
  failures.push(`${sourceGateContractPath}: production source gate must remain enabled`);
}
if (eventAwareContract?.task !== "FORUM-23A9") {
  failures.push(`${eventAwareContractPath}: expected FORUM-23A9 event-aware API`);
}
if (eventAwareContract?.mutation_api_boundary?.facade_is_publicly_exported !== true) {
  failures.push(`${eventAwareContractPath}: public mutation facade must remain exported`);
}

if (contract) {
  if (contract.task !== "FORUM-23A10") failures.push(`${contractPath}: unexpected task`);
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${contractPath}: unexpected status`);
  }
  if (contract.owner_definition_path !== servicesPath) {
    failures.push(`${contractPath}: unexpected owner definition path`);
  }
  if (contract.preferred_mutation_api !== facadePath) {
    failures.push(`${contractPath}: unexpected preferred mutation API`);
  }
  if (contract.profiles_readme !== profilesReadmePath) {
    failures.push(`${contractPath}: unexpected Profiles README path`);
  }
  if (contract.legacy_source_gate_contract !== sourceGateContractPath) {
    failures.push(`${contractPath}: unexpected source gate contract`);
  }
  if (contract.event_aware_api_contract !== eventAwareContractPath) {
    failures.push(`${contractPath}: unexpected event-aware API contract`);
  }

  const expectedMapping = Object.fromEntries(
    Object.entries(deprecatedMethods).map(([method, policy]) => [method, policy.replacement]),
  );
  if (JSON.stringify(contract.deprecated_methods) !== JSON.stringify(expectedMapping)) {
    failures.push(`${contractPath}: deprecated method mapping drift`);
  }

  for (const key of [
    "legacy_method_signatures_are_retained",
    "legacy_methods_emit_rust_deprecation_diagnostics",
    "each_diagnostic_names_the_event_aware_replacement",
    "each_diagnostic_explains_atomic_owner_write_event_coupling",
    "repository_production_call_sites_remain_forbidden",
    "graphql_self_service_uses_event_aware_facade",
    "cli_backfill_uses_event_aware_facade",
    "profile_read_and_planning_methods_are_not_deprecated",
    "legacy_backfill_internal_call_is_locally_allow_deprecated",
    "profiles_readme_documents_compatibility_shim_status",
  ]) {
    if (contract.deprecation_boundary?.[key] !== true) {
      failures.push(`${contractPath}: deprecation boundary ${key} drift`);
    }
  }

  for (const key of [
    "legacy_profile_service_methods_removed",
    "legacy_method_signatures_changed",
    "profile_read_api_changed",
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
    "legacy_methods_are_compile_time_private",
    "legacy_methods_are_event_aware",
    "external_downstream_crates_cannot_call_legacy_methods",
    "deprecation_warnings_are_denied_workspace_wide",
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
  if (contract.downstream_task !== "FORUM-23A11") {
    failures.push(`${contractPath}: unexpected downstream task`);
  }
}

if (failures.length > 0) {
  console.error("Profiles legacy mutation deprecation verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Profiles legacy mutation deprecation verification passed.");
