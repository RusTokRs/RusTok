#!/usr/bin/env node
// RBAC artifact permission owner-event and transactional-outbox guardrails.

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) {
    failures.push(`${relativePath}: expected file`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireMarker(text, marker, description) {
  const found = typeof marker === "string" ? text.includes(marker) : marker.test(text);
  if (!found) failures.push(description);
}

function forbidMarker(text, marker, description) {
  const found = typeof marker === "string" ? text.includes(marker) : marker.test(text);
  if (found) failures.push(description);
}

const paths = {
  modules: "modules.toml",
  modulesExample: "modules.toml.example",
  moduleManifest: "crates/rustok-rbac/rustok-module.toml",
  cargo: "crates/rustok-rbac/Cargo.toml",
  event: "crates/rustok-events/src/rbac_artifact_permission.rs",
  eventLib: "crates/rustok-events/src/lib.rs",
  contract: "crates/rustok-events/src/contract.rs",
  rbacLib: "crates/rustok-rbac/src/lib.rs",
  owner: "crates/rustok-rbac/src/artifact_permission_assignment.rs",
  host: "apps/server/src/controllers/artifact_permissions.rs",
  integrationTest: "crates/rustok-rbac/tests/artifact_permission_outbox_sqlite.rs",
};

const content = Object.fromEntries(
  Object.entries(paths).map(([name, relativePath]) => [name, read(relativePath)]),
);

const manifestMarker =
  'rbac = { crate = "rustok-rbac", source = "path", path = "crates/rustok-rbac", required = true, depends_on = ["outbox"] }';
requireMarker(
  content.modules,
  manifestMarker,
  `${paths.modules}: RBAC Core module must declare Outbox dependency`,
);
requireMarker(
  content.modulesExample,
  manifestMarker,
  `${paths.modulesExample}: example topology must mirror the RBAC Outbox dependency`,
);
requireMarker(
  content.moduleManifest,
  'outbox = { version_req = ">=0.1.0" }',
  `${paths.moduleManifest}: Outbox module dependency missing`,
);
forbidMarker(
  content.cargo,
  "rustok-outbox.workspace = true",
  `${paths.cargo}: RBAC owner must depend on its publisher port, not the concrete Outbox crate`,
);

for (const marker of [
  "pub enum RbacArtifactPermissionEvent",
  "AssignmentChanged",
  'event_type: "rbac.artifact_role_permission.assignment_changed"',
  "impl sealed::Sealed for RbacArtifactPermissionEvent",
  "impl EventContract for RbacArtifactPermissionEvent",
  "impl ValidateEvent for RbacArtifactPermissionEvent",
  'validate_not_nil_uuid("operation_id", operation_id)',
  "validate_max_length(",
]) {
  requireMarker(content.event, marker, `${paths.event}: event contract missing ${marker}`);
}
for (const marker of [
  "mod rbac_artifact_permission;",
  "RBAC_ARTIFACT_PERMISSION_EVENT_SCHEMAS",
  "rbac_artifact_permission_event_schema",
  ".chain(RBAC_ARTIFACT_PERMISSION_EVENT_SCHEMAS.iter())",
]) {
  requireMarker(content.eventLib, marker, `${paths.eventLib}: registry missing ${marker}`);
}
for (const marker of [
  "RbacArtifactPermission(RbacArtifactPermissionEvent)",
  "Self::RbacArtifactPermission(event) => event.event_type()",
  "Self::RbacArtifactPermission(event) => event.schema_version()",
  "Self::RbacArtifactPermission(event) => event.validate()",
]) {
  requireMarker(content.contract, marker, `${paths.contract}: sealed payload missing ${marker}`);
}
requireMarker(
  content.rbacLib,
  "ArtifactPermissionEventPublisher",
  `${paths.rbacLib}: owner publisher port must be exported`,
);

for (const marker of [
  "pub trait ArtifactPermissionEventPublisher",
  "transaction: &DatabaseTransaction",
  "event_publisher: Arc<dyn ArtifactPermissionEventPublisher>",
  "let operation_id = match insert_operation",
  "let changed = if command.granted",
  "if changed",
  ".publish_assignment_changed(",
  "assignment_event(operation_id, &command)",
  "transaction.rollback().await.map_err(database_error)?",
  "transaction.commit().await.map_err(database_error)?",
  "then_some(operation_id)",
  "Ok(result.rows_affected() == 1)",
]) {
  requireMarker(content.owner, marker, `${paths.owner}: transactional owner path missing ${marker}`);
}
forbidMarker(
  content.owner,
  "rustok_outbox",
  `${paths.owner}: owner must not import concrete Outbox types`,
);
const changedIndex = content.owner.indexOf("if changed");
const publishIndex = content.owner.indexOf(".publish_assignment_changed(");
const rollbackIndex = content.owner.indexOf(
  "transaction.rollback().await.map_err(database_error)?",
  publishIndex,
);
const commitIndex = content.owner.indexOf("transaction.commit().await.map_err(database_error)?");
if (
  !(
    changedIndex >= 0 &&
    publishIndex > changedIndex &&
    rollbackIndex > publishIndex &&
    commitIndex > rollbackIndex
  )
) {
  failures.push(
    `${paths.owner}: expected state change -> publisher port -> failure rollback -> commit order`,
  );
}
const existingIndex = content.owner.indexOf("if let Some(existing) = find_operation");
if (!(existingIndex >= 0 && existingIndex < publishIndex)) {
  failures.push(`${paths.owner}: exact retry lookup must precede event publication`);
}

for (const marker of [
  "TransactionalOutboxArtifactPermissionEventPublisher",
  "impl ArtifactPermissionEventPublisher",
  ".publish_contract_in_tx(",
  "transactional_event_bus_from_context",
  "Arc::new(TransactionalOutboxArtifactPermissionEventPublisher::new(",
  "RbacArtifactPermissionAssignmentService::new(ctx.db_clone(), event_publisher)",
  "transactional_outbox_adapter_writes_typed_event",
  '"rbac.artifact_role_permission.assignment_changed"',
]) {
  requireMarker(content.host, marker, `${paths.host}: host Outbox adapter missing ${marker}`);
}

for (const marker of [
  "impl ArtifactPermissionEventPublisher for SqliteArtifactPermissionEventPublisher",
  "only_state_changes_publish_artifact_permission_events",
  "assert_eq!(event_grants(&db).await, vec![true])",
  "assert_eq!(event_grants(&db).await, vec![true, false])",
  "publication_failure_rolls_back_grant_and_idempotency_receipt",
  "publisher failure must fail closed",
  'table_count(&db, "rbac_artifact_role_permissions")',
  'table_count(&db, "rbac_artifact_role_permission_operations")',
  'table_count(&db, "rbac_artifact_permission_events")',
]) {
  requireMarker(
    content.integrationTest,
    marker,
    `${paths.integrationTest}: executable owner regression missing ${marker}`,
  );
}
forbidMarker(
  content.integrationTest,
  "rustok_outbox",
  `${paths.integrationTest}: owner regression must remain transport-neutral`,
);

if (failures.length > 0) {
  console.error("RBAC artifact permission outbox verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("RBAC artifact permission outbox verification passed");
