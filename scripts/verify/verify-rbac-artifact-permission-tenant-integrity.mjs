#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.cwd();
const load = (file) => readFileSync(path.join(root, file), "utf8");
const failures = [];
const requireAll = (file, markers) => {
  const source = load(file);
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${file}: missing ${marker}`);
  }
  return source;
};
const forbidAll = (file, markers) => {
  const source = load(file);
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${file}: forbidden ${marker}`);
  }
};

const catalogMigration =
  "crates/rustok-rbac/src/m20260716_000001_artifact_permission_catalog.rs";
const grantMigration =
  "crates/rustok-rbac/src/m20260717_000001_artifact_role_permissions.rs";
const cutoverMigration =
  "crates/rustok-rbac/src/m20260803_000001_canonicalize_artifact_permissions.rs";
const supersededMigration =
  "crates/rustok-rbac/src/m20260801_000001_enforce_artifact_permission_tenant_integrity.rs";
const platformMigrator = "crates/rustok-migrations/src/lib.rs";
const migrationCompatibilityWorkflow = ".github/workflows/migration-compatibility.yml";
const migrationCompatibilityVerifier =
  "scripts/verify/verify-migration-plan-compatibility.mjs";
const owner = "crates/rustok-rbac/src/artifact_permission_assignment.rs";
const catalog = "crates/rustok-rbac/src/artifact_permission_catalog.rs";
const exports = "crates/rustok-rbac/src/lib.rs";
const docs = "crates/rustok-rbac/docs/README.md";
const host = "apps/server/src/controllers/artifact_permissions.rs";
const userAdmin =
  "apps/server/src/services/auth_admin_mutation_provider/user_admin.rs";
const sqliteProof =
  "crates/rustok-rbac/tests/artifact_permission_tenant_integrity_sqlite.rs";
const upgradeProof =
  "crates/rustok-rbac/tests/artifact_permission_upgrade_sqlite.rs";
const outboxProof =
  "crates/rustok-rbac/tests/artifact_permission_outbox_sqlite.rs";

if (existsSync(path.join(root, supersededMigration))) {
  failures.push(`${supersededMigration}: superseded trigger-only migration must be deleted`);
}
if (!existsSync(path.join(root, cutoverMigration))) {
  failures.push(`${cutoverMigration}: append-only canonical cutover migration is required`);
}

requireAll(exports, [
  "mod m20260803_000001_canonicalize_artifact_permissions;",
  "m20260716_000001_artifact_permission_catalog::Migration",
  "m20260717_000001_artifact_role_permissions::Migration",
  "m20260803_000001_canonicalize_artifact_permissions::Migration",
  "ArtifactPermissionAssignmentScope",
]);
forbidAll(exports, ["m20260801_000001_enforce_artifact_permission_tenant_integrity"]);

// The canonical planner may use dependency descriptors for real schema/data dependencies,
// but already-published migration IDs remain an immutable prefix. Do not resurrect the
// retired explicit tail helper or encode historical cross-module release order as a fake
// dependency merely to reproduce an old source layout.
requireAll(platformMigrator, [
  "all.sort_by(|a, b| a.name().cmp(b.name()));",
  "sort_migrations_by_dependencies(&mut all, &dependencies)",
  "validate_migration_dependency_order(&all, &dependencies)",
]);
forbidAll(platformMigrator, [
  "APPEND_ONLY_MIGRATION_TAIL",
  "move_migrations_to_append_only_tail",
]);
requireAll(migrationCompatibilityWorkflow, [
  "name: Migration Compatibility",
  "name: Append-only migration plan",
  "verify-migration-plan-compatibility.mjs",
  "Export base migration plan",
  "Export head migration plan",
]);
requireAll(migrationCompatibilityVerifier, [
  "if (head.length < base.length)",
  "if (base[index] !== head[index])",
  "migration ${index + 1} changed from",
  "migration history is append-only",
]);

// Historical migration bodies are append-only and must retain the main-branch schema.
requireAll(catalogMigration, [
  "CREATE TABLE rbac_artifact_permission_catalog",
  "scope_key TEXT NOT NULL",
  "installation_id UUID NOT NULL",
  "permission_key TEXT NOT NULL",
  "locale TEXT NOT NULL",
  "UNIQUE (scope_key, installation_id, permission_key, locale)",
]);
forbidAll(catalogMigration, [
  "rbac_artifact_permission_definitions",
  "rbac_artifact_permission_translations",
  "rustok_reject_artifact_permission_installation_update",
  "rustok_reject_artifact_permission_definition_update",
]);
requireAll(grantMigration, [
  "CREATE TABLE rbac_artifact_role_permissions",
  "installation_id UUID NOT NULL",
  "permission_key TEXT NOT NULL",
  "CREATE TABLE rbac_artifact_role_permission_operations",
  "UNIQUE (tenant_id, role_id, installation_id, permission_key)",
]);
forbidAll(grantMigration, [
  "artifact_permission_id UUID",
  "permission_scope_key TEXT",
  "uq_rbac_roles_tenant_id_id",
  "FOREIGN KEY (tenant_id, role_id)",
]);

requireAll(cutoverMigration, [
  "Append-only cutover",
  "rbac_artifact_permission_installations",
  "rbac_artifact_permission_definitions_new",
  "rbac_artifact_permission_translations_new",
  "normalize_locale_tag",
  "conflicting copy for canonical locale",
  "validate_legacy_authorization_rows",
  "ambiguous or orphan identity",
  "CREATE UNIQUE INDEX IF NOT EXISTS uq_rbac_roles_tenant_id_id",
  "CREATE UNIQUE INDEX IF NOT EXISTS uq_rbac_users_tenant_id_id",
  "artifact_permission_id UUID NOT NULL",
  "permission_scope_key TEXT NOT NULL",
  "FOREIGN KEY (tenant_id, role_id) REFERENCES roles (tenant_id, id)",
  "FOREIGN KEY (tenant_id, granted_by_actor_id) REFERENCES users (tenant_id, id)",
  "FOREIGN KEY (tenant_id, actor_id) REFERENCES users (tenant_id, id)",
  "FOREIGN KEY (artifact_permission_id, permission_scope_key) REFERENCES rbac_artifact_permission_definitions (id, scope_key)",
  "rustok_reject_artifact_permission_installation_update",
  "rustok_reject_artifact_permission_definition_update",
  "cannot roll back distinct scoped grants that collapse to one legacy key",
  "validate_rollback_legacy_selectors",
  "cannot roll back artifact permission {label} with ambiguous legacy selector",
  "TransactionTrait",
  "let transaction = connection.begin().await?",
  "apply_up(&transaction, backend)",
  "apply_down(&transaction, backend)",
  "transaction.commit().await",
  "transaction.rollback().await",
  "SQLite rollback failed",
]);
forbidAll(cutoverMigration, ["BEGIN IMMEDIATE", "finish_sqlite_transaction"]);

requireAll(catalog, [
  "const MAX_PERMISSION_KEY_LENGTH: usize = 256;",
  "normalize_locale_tag",
  "let mut permission_keys = HashSet::new()",
  "permission.key.len() > MAX_PERMISSION_KEY_LENGTH",
  "permission.key.trim() != permission.key",
  "permission.key.chars().any(char::is_control)",
  "!permission_keys.insert(permission.key.as_str())",
  "let mut normalized_locales = HashSet::new()",
  "!normalized_locales.insert(normalized_locale)",
  "ensure_installation_identity",
  "rbac_artifact_permission_installations",
  "installation_insert_sql",
  "installation_select_sql",
  "definition_insert_sql",
  "definition_select_sql",
  "translation_upsert_sql",
  "rbac.artifact_permission_identity_conflict",
  "ArtifactPermissionScope::Tenant { tenant_id } if tenant_id.is_nil()",
  "registration_normalizes_locale_and_is_idempotent",
  "registration_rejects_installation_scope_rebinding",
  "registration_rejects_nil_tenant_scope",
  "registration_rejects_duplicate_normalized_locales",
  "registration_rejects_unassignable_or_duplicate_permission_keys",
]);
requireAll(owner, [
  "pub enum ArtifactPermissionAssignmentScope",
  "pub scope: ArtifactPermissionAssignmentScope",
  "pub installation_id: Uuid",
  "pub permission_key: String",
  "scope_key(command.scope, command.tenant_id)",
  "ArtifactPermissionAssignmentScope::Platform => \"platform\".to_string()",
  "ArtifactPermissionAssignmentScope::Tenant => format!(\"tenant:{tenant_id}\")",
  "struct ArtifactPermissionIdentity",
  "resolve_artifact_permission_identity(&transaction, &command).await?",
  "SELECT id, scope_key, installation_id, permission_key FROM rbac_artifact_permission_definitions WHERE scope_key = {scope_key} AND installation_id = {installation_id} AND permission_key = {permission_key}",
  "permission_scope_key != artifact_permission.scope_key",
  "INNER JOIN rbac_artifact_permission_definitions apd ON apd.id = arp.artifact_permission_id AND apd.scope_key = arp.permission_scope_key",
]);
forbidAll(owner, [
  "permission_is_registered",
  "rbac_artifact_permission_catalog",
  "ORDER BY CASE WHEN scope_key",
  "WHERE id = {artifact_permission_id}",
  "pub artifact_permission_id: Uuid",
]);

requireAll(host, [
  "pub(crate) enum ArtifactPermissionAssignmentScopeRequest",
  "pub scope: ArtifactPermissionAssignmentScopeRequest",
  "pub installation_id: Uuid",
  "pub permission_key: String",
  "scope: input.scope.into()",
  "request_scope_maps_without_accepting_a_tenant_identifier",
  "Role or permission in the requested explicit scope not found",
]);

requireAll(sqliteProof, [
  "migrations.len(),\n        5",
  "artifact integrity cutover must append after unchanged historical migrations",
  "database_rejects_cross_tenant_scope_and_orphan_artifact_state",
  "PRAGMA foreign_keys = ON",
  "permission_scope_key",
  "platform permission may be granted in any tenant",
]);
requireAll(upgradeProof, [
  "legacy_catalog_grant_and_receipt_upgrade_and_rollback_truthfully",
  "apply historical RBAC migration",
  "upgrade legacy artifact authorization state",
  "SELECT locale FROM rbac_artifact_permission_translations",
  '"en-US"',
  "roll back append-only cutover",
  "legacy_installation_with_platform_and_tenant_scope_fails_closed_atomically",
  "ambiguous legacy selector must fail closed",
  "rbac_artifact_permission_installations",
  "rbac_artifact_permission_definitions_new",
  "canonical_grant_with_later_scope_collision_fails_rollback",
  "canonical_receipt_with_later_scope_collision_fails_rollback",
  "grant with ambiguous legacy selector",
  "operation receipt with ambiguous legacy selector",
  "late_sqlite_down_failure_restores_canonical_schema",
  "reserve legacy index name to force a late rollback failure",
  "rbac_artifact_permission_catalog_restore",
]);
requireAll(outboxProof, [
  "explicit_scope_mutation_remains_exact_for_corrupt_parallel_definitions",
  "grant explicit permission scope",
  "revoke explicit platform scope",
  "revoke explicit tenant scope",
  "assert_eq!(remaining_id, tenant_permission_id)",
  "assert_ne!(remaining_id, platform_permission_id)",
]);

requireAll(userAdmin, [
  "async fn delete_user(",
  "AuthLifecycleService::deactivate_user_in_tx",
  "redact_profile_for_account_deactivation_in_tx",
]);
forbidAll(userAdmin, ["users::Entity::delete", "DELETE FROM users"]);
requireAll(docs, [
  "## Artifact authorization lifecycle and teardown",
  "the current Auth Admin `delete_user` operation is account",
  "A future hard-delete",
  "Until that workflow exists, `RESTRICT` is the canonical",
]);

if (failures.length > 0) {
  console.error("RBAC artifact permission tenant-integrity verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("RBAC artifact permission tenant-integrity source contract verified");
