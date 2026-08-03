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
const correctiveMigration =
  "crates/rustok-rbac/src/m20260801_000001_enforce_artifact_permission_tenant_integrity.rs";
const owner = "crates/rustok-rbac/src/artifact_permission_assignment.rs";
const catalog = "crates/rustok-rbac/src/artifact_permission_catalog.rs";
const exports = "crates/rustok-rbac/src/lib.rs";
const docs = "crates/rustok-rbac/docs/README.md";
const host = "apps/server/src/controllers/artifact_permissions.rs";
const userAdmin =
  "apps/server/src/services/auth_admin_mutation_provider/user_admin.rs";
const sqliteProof =
  "crates/rustok-rbac/tests/artifact_permission_tenant_integrity_sqlite.rs";
const outboxProof =
  "crates/rustok-rbac/tests/artifact_permission_outbox_sqlite.rs";

if (existsSync(path.join(root, correctiveMigration))) {
  failures.push(`${correctiveMigration}: superseded corrective migration must be deleted`);
}
requireAll(exports, [
  "m20260716_000001_artifact_permission_catalog::Migration",
  "m20260717_000001_artifact_role_permissions::Migration",
  "ArtifactPermissionAssignmentScope",
]);
forbidAll(exports, ["m20260801_000001_enforce_artifact_permission_tenant_integrity"]);

requireAll(catalogMigration, [
  "CREATE TABLE rbac_artifact_permission_definitions",
  "UNIQUE (id, scope_key)",
  "UNIQUE (scope_key, installation_id, permission_key)",
  "CREATE TRIGGER rbac_artifact_permission_definitions_immutable",
  "rustok_reject_artifact_permission_definition_update",
  "CREATE TABLE rbac_artifact_permission_translations",
  "locale VARCHAR(32) NOT NULL",
  "REFERENCES rbac_artifact_permission_definitions (id)",
]);
forbidAll(catalogMigration, ["CREATE TABLE rbac_artifact_permission_catalog"]);

requireAll(grantMigration, [
  "uq_rbac_roles_tenant_id_id",
  "uq_rbac_users_tenant_id_id",
  "permission_scope_key TEXT NOT NULL",
  "permission_scope_key = 'platform' OR permission_scope_key = 'tenant:'",
  "FOREIGN KEY (tenant_id, role_id) REFERENCES roles (tenant_id, id)",
  "FOREIGN KEY (tenant_id, granted_by_actor_id) REFERENCES users (tenant_id, id)",
  "FOREIGN KEY (tenant_id, actor_id) REFERENCES users (tenant_id, id)",
  "FOREIGN KEY (artifact_permission_id, permission_scope_key) REFERENCES rbac_artifact_permission_definitions (id, scope_key)",
  "ON UPDATE RESTRICT ON DELETE RESTRICT",
  "UNIQUE (tenant_id, role_id, artifact_permission_id)",
]);
forbidAll(grantMigration, [
  "rustok_enforce_artifact_role_permission_integrity",
  "trg_rbac_artifact_role_permissions_integrity",
]);

requireAll(catalog, [
  "normalize_locale_tag",
  "let mut normalized_locales = HashSet::new()",
  "!normalized_locales.insert(normalized_locale)",
  "definition_insert_sql",
  "definition_select_sql",
  "translation_upsert_sql",
  "rbac.artifact_permission_identity_conflict",
  "ArtifactPermissionScope::Tenant { tenant_id } if tenant_id.is_nil()",
  "registration_normalizes_locale_and_is_idempotent_without_role_tables",
  "registration_rejects_nil_tenant_scope",
  "registration_rejects_duplicate_normalized_locales",
  "assert_eq!(locale, \"en-US\")",
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
  "permission_scope_key: String",
  "permission_scope_key != artifact_permission.scope_key",
  "INNER JOIN rbac_artifact_permission_definitions apd ON apd.id = arp.artifact_permission_id AND apd.scope_key = arp.permission_scope_key",
  "ON CONFLICT (tenant_id, role_id, artifact_permission_id) DO NOTHING",
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
  "artifact integrity must remain consolidated in canonical migrations",
  "database_rejects_cross_tenant_scope_and_orphan_artifact_state",
  "PRAGMA foreign_keys = ON",
  "permission_scope_key",
  "platform permission may be granted in any tenant",
  "foreign-scope-op",
  "UPDATE roles SET tenant_id",
  "UPDATE users SET tenant_id",
  "DELETE FROM roles WHERE id",
  "DELETE FROM users WHERE id",
  "UPDATE rbac_artifact_permission_definitions SET permission_key",
  "UPDATE rbac_artifact_permission_definitions SET scope_key",
  "DELETE FROM rbac_artifact_permission_definitions",
]);
requireAll(outboxProof, [
  "explicit_scope_mutation_does_not_shadow_platform_or_tenant_definition",
  "grant explicit permission scope",
  "revoke explicit platform scope",
  "revoke explicit tenant scope",
  "assert_eq!(remaining_id, tenant_permission_id)",
  "assert_ne!(remaining_id, platform_permission_id)",
  "assert_eq!(event_grants(&db).await, vec![true, true, false, false])",
]);

requireAll(userAdmin, [
  "async fn delete_user(",
  "AuthLifecycleService::deactivate_user_in_tx",
  "redact_profile_for_account_deactivation_in_tx",
]);
forbidAll(userAdmin, ["users::Entity::delete", "DELETE FROM users"]);
requireAll(docs, [
  "## Artifact authorization lifecycle and teardown",
  "The current Auth Admin `delete_user` operation is account deactivation",
  "A future hard-delete workflow must run in one owner-coordinated transaction",
  "Until that workflow exists, `RESTRICT` is the canonical behavior",
]);

if (failures.length > 0) {
  console.error("RBAC artifact permission tenant-integrity verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("RBAC artifact permission tenant-integrity source contract verified");
