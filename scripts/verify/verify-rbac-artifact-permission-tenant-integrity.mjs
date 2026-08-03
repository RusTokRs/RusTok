#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const read = (relative) => readFileSync(path.join(root, relative), "utf8");
const requireText = (source, text, label) => {
  if (!source.includes(text)) {
    throw new Error(`${label}: missing ${JSON.stringify(text)}`);
  }
};

const migration = read(
  "crates/rustok-rbac/src/m20260801_000001_enforce_artifact_permission_tenant_integrity.rs",
);
const exports = read("crates/rustok-rbac/src/lib.rs");
const owner = read("crates/rustok-rbac/src/artifact_permission_assignment.rs");
const sqliteProof = read(
  "crates/rustok-rbac/tests/artifact_permission_tenant_integrity_sqlite.rs",
);

for (const text of [
  "m20260801_000001_enforce_artifact_permission_tenant_integrity",
  "m20260717_000001_artifact_role_permissions::Migration",
  "m20260801_000001_enforce_artifact_permission_tenant_integrity::Migration",
]) {
  requireText(exports, text, "migration registration");
}
const tableMigration = exports.indexOf(
  "m20260717_000001_artifact_role_permissions::Migration",
);
const integrityMigration = exports.indexOf(
  "m20260801_000001_enforce_artifact_permission_tenant_integrity::Migration",
);
if (!(tableMigration < integrityMigration)) {
  throw new Error("artifact integrity migration must run after artifact tables exist");
}

for (const text of [
  "DELETE FROM rbac_artifact_role_permissions",
  "DELETE FROM rbac_artifact_role_permission_operations",
  "rustok_enforce_artifact_role_permission_integrity",
  "rustok_enforce_artifact_role_permission_operation_integrity",
  "trg_rbac_users_tenant_update",
  "trg_rbac_roles_tenant_update",
  "trg_rbac_artifact_role_delete",
  "trg_rbac_artifact_actor_delete",
  "trg_rbac_artifact_permission_catalog_identity_update",
  "trg_rbac_artifact_permission_catalog_delete",
  "RBAC referenced artifact permission identity is immutable",
  "sqlite_trigger_names() -> [&'static str; 10]",
  "sqlite_triggers() -> [&'static str; 10]",
]) {
  requireText(migration, text, "artifact tenant integrity migration");
}

const existing = owner.indexOf("find_operation(&transaction, &command)");
const role = owner.indexOf("if !role_exists(&transaction, &command).await?");
const permission = owner.indexOf(
  "if !permission_is_registered(&transaction, &command).await?",
);
const insert = owner.indexOf("insert_operation(&transaction, &command)");
if (!(existing < role && role < permission && permission < insert)) {
  throw new Error(
    "owner ordering must be existing receipt -> typed role/catalog validation -> receipt insert",
  );
}
requireText(
  owner,
  "database-integrity\n        // triggers preserve the stable RoleNotFound/PermissionNotRegistered contract",
  "stable typed error contract",
);

for (const text of [
  "migration_cleans_legacy_malformed_artifact_rows",
  "database_rejects_cross_tenant_and_orphan_artifact_state",
  "module_slug, release_digest, permission_key, locale, label, description, registered_at",
  "UPDATE rbac_artifact_permission_catalog SET label = 'Updated label'",
  "UPDATE roles SET tenant_id",
  "UPDATE users SET tenant_id",
  "DELETE FROM roles WHERE id",
  "DELETE FROM users WHERE id",
  "UPDATE rbac_artifact_permission_catalog SET permission_key",
  "DELETE FROM rbac_artifact_permission_catalog",
]) {
  requireText(sqliteProof, text, "SQLite artifact integrity proof");
}

console.log("RBAC artifact permission tenant-integrity source contract verified");
