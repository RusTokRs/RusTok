import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-reactions/contracts/reactions-owner-persistence.json";

function fail(message) {
  throw new Error(`Reactions owner persistence verification failed: ${message}`);
}

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) fail(`missing required file ${relativePath}`);
  return readFileSync(absolute, "utf8");
}

const contract = JSON.parse(read(contractPath));
if (contract.schema_version !== 1) fail("unsupported contract schema version");
if (contract.contract !== "reactions_owner_persistence_v1") {
  fail("unexpected contract identity");
}

const cargo = read("crates/rustok-reactions/Cargo.toml");
const lib = read("crates/rustok-reactions/src/lib.rs");
const entities = read("crates/rustok-reactions/src/entities.rs");
const migrationIndex = read("crates/rustok-reactions/src/migrations/mod.rs");
const migration = read(
  "crates/rustok-reactions/src/migrations/m20260806_000001_create_reaction_owner_state.rs",
);
const service = read("crates/rustok-reactions/src/service.rs");
const plan = read("crates/rustok-reactions/docs/implementation-plan.md");
const forumPlan = read("crates/rustok-forum/docs/implementation-plan.md");

for (const dependency of [
  "rustok-api.workspace = true",
  "rustok-outbox.workspace = true",
  "sea-orm.workspace = true",
  "sea-orm-migration.workspace = true",
  "serde_json.workspace = true",
]) {
  if (!cargo.includes(dependency)) fail(`Cargo boundary is missing ${dependency}`);
}

for (const table of contract.tables) {
  if (!entities.includes(`table_name = "${table}"`)) {
    fail(`owner entity is missing table ${table}`);
  }
}

for (const fragment of [
  "ux_reaction_subjects_tenant_identity",
  "ux_reaction_subjects_tenant_id",
  "fk_reaction_catalogs_tenant_subject",
  "fk_reaction_actor_states_tenant_subject",
  "fk_reaction_aggregates_tenant_subject",
  "ux_reaction_catalogs_tenant_subject_revision",
  "ux_reaction_actor_states_tenant_subject_actor",
  "ux_reaction_aggregates_tenant_subject_key",
]) {
  if (!migration.includes(fragment)) fail(`migration is missing ${fragment}`);
}

if (!migrationIndex.includes(contract.migration)) {
  fail("migration index does not register the owner migration");
}
if (!migrationIndex.includes(contract.migration_dependency)) {
  fail("owner migration does not depend on the shared receipt migration");
}
for (const fragment of [
  "impl MigrationSource for ReactionsModule",
  "migrations::migrations()",
  "migrations::migration_dependencies()",
]) {
  if (!lib.includes(fragment)) fail(`module boundary is missing ${fragment}`);
}

for (const fragment of [
  "impl ReactionReadPort for ReactionsService",
  "impl ReactionWritePort for ReactionsService",
  "PortCallPolicy::read()",
  "PortCallPolicy::write()",
  "reactions.command_idempotency_mismatch",
  "idempotency::admit(",
  ".authorize(context.clone(),",
  "synchronize_subject(",
  "self.database.begin()",
  "idempotency::complete(&transaction",
  "transaction.commit()",
  "ReactionSelectionPolicy::Single",
  "ReactionSelectionPolicy::Multiple",
  "reactions.selection_limit_reached",
  "reactions.aggregate_count_underflow",
  "reactions.catalog_revision_rebound",
  "reactions.catalog_reconciliation_required",
  "reactions:act_as_actor",
]) {
  if (!service.includes(fragment)) fail(`owner service is missing ${fragment}`);
}

for (const forbidden of [
  "rustok_forum",
  "rustok_blog",
  "rustok_comments",
  "rustok_profiles",
  "rustok_media",
  "async_graphql",
  "axum::",
  "leptos::",
]) {
  if (service.includes(forbidden) || entities.includes(forbidden)) {
    fail(`owner persistence imports forbidden dependency ${forbidden}`);
  }
}

for (const fragment of [
  "| `REACTIONS-01` | `in_progress` |",
  "| `REACTIONS-02` | `in_progress` |",
  "shared Outbox",
  "Forum `topic` and `reply` `ReactionSubjectProvider`",
]) {
  if (!plan.includes(fragment)) fail(`Reactions plan is missing ${fragment}`);
}

for (const fragment of [
  "| `FORUM-18` | `in_progress` |",
  "tenant-composite persistence",
  "Forum `topic` and `reply`",
  "`ReactionSubjectProvider`",
  "Existing Forum votes remain Forum semantics",
]) {
  if (!forumPlan.includes(fragment)) fail(`Forum plan is missing ${fragment}`);
}

console.log("Reactions owner persistence source contract verified.");
