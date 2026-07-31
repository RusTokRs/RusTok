import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const files = {
  composition: "apps/server/src/services/index_replay_runtime_composition.rs",
  base: "apps/server/src/services/index_replay_runtime_composition_base.rs",
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
  "#[path = \"index_replay_runtime_composition_base.rs\"]",
  "pub struct IndexReconciliationDeadLetterOperatorRuntime",
  "permissions_for(&context.tenant_id(), &context.actor_id())",
  "Permission::MODULES_MANAGE",
  "inspect_dead_letter(",
  ".inspect(context.tenant_id(), job_id)",
  "base::materialize_index_replay_runtime(extensions, db.clone())?",
  "extensions.contains::<IndexReconciliationOperatorRuntime>()",
  "PostgresIndexReconciliationDeadLetterInspector::new(db)",
  "dead_letter_inspection_requires_request_bound_authority_before_database_access",
  "dead_letter_inspection_requires_modules_manage",
  "authorized_dead_letter_inspection_uses_context_tenant_and_delegates",
]);

requireMarkers("base", [
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
  "context.authorize_for(request.tenant_id())?",
  "request_cancel(context.tenant_id(), job_id)",
  "PostgresIndexReconciliationRunner::new(db, sources, schemas.shared())",
]);

requireMarkers("docs", [
  "Status: implementation retained, not run.",
  "Run, cancellation, and dead-letter inspection require `Permission::MODULES_MANAGE`.",
  "derive tenant scope only from the authorized `IndexReconciliationOperatorContext`",
  "publishes the dead-letter inspection operator",
  "does not add audit records",
  "The canonical M6 reconciliation and drift-repair item therefore remains open.",
]);

forbidMarkers("composition", [
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

console.log("Index server reconciliation run/cancel/dead-letter guards verified.");
