import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const files = {
  composition: "apps/server/src/services/index_replay_runtime_composition.rs",
  operator: "apps/server/src/services/index_reconciliation_operator.rs",
  docs: "apps/server/docs/index-reconciliation-operator-runtime.md",
};

const read = (name) => {
  const relative = files[name];
  const absolute = path.join(root, relative);
  if (!fs.existsSync(absolute)) {
    throw new Error(`missing Index server reconciliation guard file: ${relative}`);
  }
  return fs.readFileSync(absolute, "utf8");
};

const requireMarkers = (name, markers) => {
  const content = read(name);
  for (const marker of markers) {
    if (!content.includes(marker)) {
      throw new Error(`${files[name]} is missing required marker: ${marker}`);
    }
  }
};

const forbidMarkers = (name, markers) => {
  const content = read(name);
  for (const marker of markers) {
    if (content.includes(marker)) {
      throw new Error(`${files[name]} contains forbidden marker: ${marker}`);
    }
  }
};

requireMarkers("composition", [
  "#[path = \"index_reconciliation_operator.rs\"]",
  "IndexReconciliationOperatorRuntime",
  "materialize_postgres_index_replay_runtime(extensions, db.clone())",
  "materialize_index_reconciliation_operator(extensions, db)?",
]);

requireMarkers("operator", [
  "pub struct IndexReconciliationOperatorContext",
  "tenant_id.is_nil() || actor_id.is_nil()",
  "permissions_for(&self.tenant_id, &self.actor_id)",
  "Permission::MODULES_MANAGE",
  "IndexReconciliationOperatorError::TenantMismatch",
  "context.authorize_for(request.tenant_id())?",
  "request_cancel(context.tenant_id(), job_id)",
  "PostgresIndexReconciliationRunner::new(db, sources, schemas.shared())",
  "missing_sources_do_not_publish_false_reconciliation_capability",
  "source_registry_without_shared_schema_registry_fails_closed",
  "complete_registries_publish_guarded_runtime_to_host_context",
  "duplicate_guarded_reconciliation_materialization_fails_closed",
  "cross_tenant_run_is_denied_before_database_access",
]);

requireMarkers("docs", [
  "Status: implementation retained, not run.",
  "requires exactly `Permission::MODULES_MANAGE`",
  "tenant comparison occurs before the inner reconciliation runner",
  "does not accept a caller-supplied tenant separate from the context",
  "performs no database I/O during composition",
  "The canonical M6 reconciliation and drift-repair item therefore remains open.",
]);

forbidMarkers("operator", [
  "tokio::spawn",
  "spawn_blocking",
  "SELECT ",
  "INSERT ",
  "UPDATE ",
  "DELETE ",
  "Router::new",
  "route(",
  "async_graphql",
]);

console.log("Index server reconciliation guard markers verified.");
