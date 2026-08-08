import fs from "node:fs";

const graphql = fs.readFileSync(
  "crates/rustok-groups/src/graphql_membership_enforcement.rs",
  "utf8",
);
const finalRoot = fs.readFileSync(
  "crates/rustok-groups/src/graphql_application_cas.rs",
  "utf8",
);
const moduleSource = fs.readFileSync("crates/rustok-groups/src/lib.rs", "utf8");
const manifest = fs.readFileSync("crates/rustok-groups/rustok-module.toml", "utf8");
const registry = fs.readFileSync(
  "crates/rustok-groups/contracts/groups-fba-registry.json",
  "utf8",
);
const plan = fs.readFileSync(
  "crates/rustok-groups/docs/implementation-plan.md",
  "utf8",
);
const docs = fs.readFileSync(
  "crates/rustok-groups/docs/membership-enforcement-graphql-contract.md",
  "utf8",
);
const sqliteParity = fs.readFileSync(
  "apps/server/tests/groups_membership_enforcement_graphql_sqlite.rs",
  "utf8",
);
const postgresParity = fs.readFileSync(
  "apps/server/tests/groups_membership_enforcement_graphql_postgres.rs",
  "utf8",
);

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

for (const marker of [
  "GroupsMembershipEnforcementMutation",
  "suspend_group_membership",
  "revoke_group_membership_suspension",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  "GroupMembershipEnforcementCommandPort::revoke_membership_suspension",
  "GroupMembershipEnforcementCommandService",
  "HostRuntimeContext",
  "AuthContext",
  "TenantContext",
  "PortActor::user",
  ".with_deadline(PORT_DEADLINE)",
  ".with_idempotency_key(idempotency_key)",
  "with_claim(permission.to_string())",
  "expected_membership_revision",
  "effective_until",
  "membership_revision",
  "group_version",
  "member_count",
  "enforcement_revision",
  "replayed",
  'const DOMAIN_CODE_EXTENSION: &str = "domainCode"',
  'const RETRYABLE_EXTENSION: &str = "retryable"',
  "let domain_code = error.code.clone()",
  "let retryable = error.retryable",
  "transport_error.extend_with",
  "extensions.set(DOMAIN_CODE_EXTENSION, domain_code)",
  "extensions.set(RETRYABLE_EXTENSION, retryable)",
  "graphql_conflict_preserves_transport_and_owner_codes",
  "graphql_unavailable_keeps_owner_code_and_retryability",
  'Some("BAD_USER_INPUT".to_string())',
  'Some("INTERNAL_ERROR".to_string())',
  'Some("groups.membership_enforcement_revision_conflict".to_string())',
  'Some("groups.persistence_unavailable".to_string())',
]) {
  requireText(graphql, marker, `Groups enforcement GraphQL source is missing ${marker}`);
}

for (const forbidden of [
  "membership_enforcement::ActiveModel",
  "group::ActiveModel",
  "Statement::",
  "execute_unprepared",
  "UPDATE group_memberships",
  "UPDATE group_membership_enforcements",
  "INSERT INTO group_membership_enforcements",
  "GroupMembershipEnforcementSourceKind::ModerationDecision",
]) {
  if (graphql.includes(forbidden)) {
    throw new Error(`Groups enforcement GraphQL transport contains owner bypass ${forbidden}`);
  }
}

for (const marker of [
  "MergedObject",
  "pub struct GroupsMutationRoot",
  "GroupsPreApplicationMutationRoot",
  "GroupsApplicationCasMutation",
  "GroupsApplicationBulkReviewMutation",
  "GroupsApplicationLifecycleMutation",
  "GroupsMembershipEnforcementMutation",
]) {
  requireText(finalRoot, marker, `Stable Groups final GraphQL root is missing ${marker}`);
}

requireText(
  moduleSource,
  '#[cfg(feature = "graphql")]\npub mod graphql_membership_enforcement;',
  "Groups module registration is missing enforcement GraphQL module",
);

for (const marker of [
  'query = "graphql_application_cas::GroupsQueryRoot"',
  'mutation = "graphql_application_cas::GroupsMutationRoot"',
]) {
  requireText(manifest, marker, `Groups stable module GraphQL composition is missing ${marker}`);
}

for (const marker of [
  '"transport_status": "rust_and_graphql_source"',
  '"graphql_root": "graphql_application_cas::GroupsMutationRoot"',
  '"suspendGroupMembership"',
  '"revokeGroupMembershipSuspension"',
  '"membership_enforcement_graphql_static_boundary"',
  '"membership_enforcement_command_transport_parity": null',
]) {
  requireText(registry, marker, `Groups FBA GraphQL contract is missing ${marker}`);
}

for (const marker of [
  "Source-complete direct enforcement GraphQL transport",
  "graphql_application_cas::GroupsMutationRoot",
  "GroupsMembershipEnforcementMutation",
  "suspendGroupMembership",
  "revokeGroupMembershipSuspension",
  "membership-enforcement-graphql.mjs",
  "Runtime schema execution",
]) {
  requireText(plan, marker, `Canonical Groups plan is missing ${marker}`);
}

for (const marker of [
  "Transport boundary",
  "Owner-only business semantics",
  "Consolidated parity contract",
  "Suspend and same-key replay",
  "Fresh stale-CAS parity",
  "Revoke and same-key replay",
  "Historical suspension replay after revoke",
  "Final owner state",
  "SQLite source",
  "PostgreSQL source",
  "No fallback",
  "graphql_application_cas::GroupsMutationRoot",
  "GroupMembershipEnforcementCommandPort",
  "domainCode",
  "retryable",
  "membership_enforcement_command_transport_parity",
  "execution pending",
  "options=-csearch_path=<schema>,public",
  "cargo check -p rustok-groups --features graphql",
  "groups_membership_enforcement_graphql_sqlite",
  "groups_membership_enforcement_graphql_postgres",
]) {
  requireText(docs, marker, `Groups enforcement GraphQL handoff is missing ${marker}`);
}

for (const marker of [
  '#![cfg(feature = "mod-groups")]',
  "tempfile::tempdir()",
  "mode=rwc",
  "rustok_groups::migrations::migrations()",
  "GroupsQueryRoot::default()",
  "GroupsMutationRoot::default()",
  "HostRuntimeContext::new(db)",
  "permissions: Vec::new()",
  "GroupMembershipEnforcementMutationResult",
  "PortErrorKind::Conflict",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  "GroupMembershipEnforcementCommandPort::revoke_membership_suspension",
  "suspendGroupMembership",
  "revokeGroupMembershipSuspension",
  "native_suspend_replay.replayed",
  "native_revoke_replay.replayed",
  "native_suspend_after_revoke.replayed",
  "graphql_stale_error.message, native_stale.message",
  'Some("BAD_USER_INPUT".to_string())',
  'Some("groups.membership_enforcement_revision_conflict".to_string())',
  'extension_json(graphql_stale_error, "domainCode")',
  'extension_json(graphql_stale_error, "retryable")',
  "native_revoke.group_version, native_suspend.group_version + 1",
  "native_final, graphql_final",
  "native_final.4, 3",
  'native_final.6, "direct_local"',
  "native_final.7, 1",
]) {
  requireText(sqliteParity, marker, `Groups enforcement GraphQL SQLite parity source is missing ${marker}`);
}

for (const forbidden of [
  "GroupsMembershipEnforcementMutation::default()",
  "groups:moderate",
  "UPDATE group_membership_enforcements",
  "INSERT INTO group_membership_enforcements",
  "GroupMembershipEffectiveState {",
  "rustok_moderation::",
  "MembershipEnforcementProvenance",
]) {
  if (sqliteParity.includes(forbidden)) {
    throw new Error(`Groups enforcement GraphQL SQLite parity source contains shortcut ${forbidden}`);
  }
}

for (const marker of [
  '#![cfg(feature = "mod-groups")]',
  '#[ignore = "requires RUSTOK_GROUPS_TEST_POSTGRES_URL"]',
  "RUSTOK_GROUPS_TEST_POSTGRES_URL",
  "options=-csearch_path%3D",
  "CREATE SCHEMA",
  "DROP SCHEMA",
  "rustok_groups::migrations::migrations()",
  "GroupsQueryRoot::default()",
  "GroupsMutationRoot::default()",
  "HostRuntimeContext::new(db)",
  "permissions: Vec::new()",
  "GroupMembershipEnforcementMutationResult",
  "PortErrorKind::Conflict",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  "GroupMembershipEnforcementCommandPort::revoke_membership_suspension",
  "suspendGroupMembership",
  "revokeGroupMembershipSuspension",
  "postgres-graphql-suspend",
  "postgres-graphql-stale-suspend",
  "postgres-graphql-revoke",
  "native_suspend_replay.replayed",
  "native_revoke_replay.replayed",
  "native_suspend_after_revoke.replayed",
  "graphql_stale_error.message, native_stale.message",
  'Some("BAD_USER_INPUT".to_string())',
  'Some("groups.membership_enforcement_revision_conflict".to_string())',
  'extension_json(graphql_stale_error, "domainCode")',
  'extension_json(graphql_stale_error, "retryable")',
  "native_revoke.group_version, native_suspend.group_version + 1",
  "native_final, graphql_final",
  "native_final.4, 3",
  'native_final.6, "direct_local"',
  "native_final.7, 1",
]) {
  requireText(postgresParity, marker, `Groups enforcement GraphQL PostgreSQL parity source is missing ${marker}`);
}

for (const forbidden of [
  "SET search_path",
  "GroupsMembershipEnforcementMutation::default()",
  "groups:moderate",
  "UPDATE group_membership_enforcements",
  "INSERT INTO group_membership_enforcements",
  "GroupMembershipEffectiveState {",
  "rustok_moderation::",
  "MembershipEnforcementProvenance",
]) {
  if (postgresParity.includes(forbidden)) {
    throw new Error(`Groups enforcement GraphQL PostgreSQL parity source contains shortcut ${forbidden}`);
  }
}

console.log("Groups membership enforcement GraphQL source and consolidated SQLite/PostgreSQL parity guard passed");
