#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? path.resolve(configuredRoot)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const read = (relativePath) => readFileSync(path.join(root, relativePath), "utf8");
const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const normalizeWhitespace = (value) => value.replace(/\s+/g, " ").trim();
const requireNormalizedText = (source, value, label) => {
  if (!normalizeWhitespace(source).includes(normalizeWhitespace(value))) {
    failures.push(`${label}: missing ${value}`);
  }
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const files = {
  harness: "apps/server/tests/rbac_postgres_concurrency.rs",
  committed: "apps/server/src/services/rbac_committed_mutations.rs",
  generation: "crates/rustok-rbac/src/invalidation_generation.rs",
  testDb: "crates/rustok-test-utils/src/db.rs",
  evidence:
    "crates/rustok-rbac/contracts/evidence/rbac-postgres-concurrency-source.json",
  docs: "crates/rustok-rbac/docs/postgres-concurrency-evidence.md",
  plan: "crates/rustok-rbac/docs/implementation-plan.md",
  master: "docs/verification/PLATFORM_VERIFICATION_PLAN.md",
};
const sources = Object.fromEntries(
  Object.entries(files).map(([name, relativePath]) => [name, read(relativePath)]),
);

for (const marker of [
  '#[ignore = "requires PostgreSQL admin access"]',
  "concurrent_role_replacement_serializes_one_target_and_advances_two_generations",
  "concurrent_super_admin_demotions_preserve_one_active_super_admin",
  "concurrent_generation_reservations_are_unique_contiguous_and_committed",
  'const RBAC_POSTGRES_ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL"',
  "unique_postgres_database_name(prefix)",
  "create_postgres_database(&admin, &database_name)",
  "drop_postgres_database_if_exists(&admin, &database_name)",
  "Migrator::up(&db_a, None).await?",
  "let db_b = connect_postgres(&target_url).await?",
  "Barrier::new(2)",
  "Barrier::new(8)",
  "barrier_a.wait().await",
  "barrier_b.wait().await",
  "barrier.wait().await",
  "RbacService::replace_user_role_committed",
  "RbacRoleAssignmentDbWriter::new(db_a.clone())",
  'slug: Set(format!("rbac-pg-{tenant_id}"))',
  "assignments.len() != 1",
  "generation_before + 2",
  "cannot demote the last active super administrator",
  "remaining_super_admins.len() != 1",
  "generation_before + 1",
  "reserve_permission_invalidation_generation(&transaction)",
  "generations.sort_unstable()",
  "generation_before + 1..=generation_before + 8",
  "generation_before + 8",
]) requireText(sources.harness, marker, `${files.harness}: PostgreSQL evidence harness`);

for (const forbidden of [
  "sqlite:",
  "setup_test_db_with_migrations",
  "UPDATE rbac_invalidation_state",
  "SELECT FOR UPDATE",
  "lock_exclusive()",
  "connect_for_assertions",
  "RUSTOK_RBAC_ACTIVE_TEST_DATABASE",
  "tokio::time::sleep",
  'rbac-postgres-{suffix}-{tenant_id}',
]) forbidText(sources.harness, forbidden, `${files.harness}: shortcut`);

for (const marker of [
  "lock_target_user_for_role_mutation",
  "lock_exclusive().one(db).await?",
  "ensure_active_super_admin_continuity",
  "reserve_rbac_invalidation_generation(&tx)",
  "tx.commit().await?",
]) requireText(sources.committed, marker, `${files.committed}: production mutation path`);

for (const marker of [
  "pub async fn reserve_permission_invalidation_generation(",
  "db: &DatabaseTransaction",
  "UPDATE rbac_invalidation_state",
  "SET generation = generation + 1",
  "if update.rows_affected() != 1",
  "read_permission_invalidation_generation(db).await",
]) requireText(sources.generation, marker, `${files.generation}: generation authority`);

for (const marker of [
  "pub async fn connect_postgres",
  "pub async fn create_postgres_database",
  "pub async fn drop_postgres_database_if_exists",
  "pub fn postgres_database_url",
  "pub fn unique_postgres_database_name",
]) requireText(sources.testDb, marker, `${files.testDb}: isolated database helper`);

const evidence = JSON.parse(sources.evidence);
const evidenceChecks = [
  [evidence.status === "source_ready_unvalidated", "source-shape status must remain source_ready_unvalidated"],
  [evidence.cycle === "cycle-001", "cycle must remain cycle-001"],
  [evidence.component === "core/rbac", "component must remain core/rbac"],
  [evidence.fixture?.backend === "postgresql", "fixture backend must be PostgreSQL"],
  [evidence.fixture?.isolated_database_per_test === true, "isolated databases must remain required"],
  [evidence.fixture?.two_independent_database_connections === true, "two connections must remain required"],
  [evidence.fixture?.barrier_synchronized_starts === true, "barrier-synchronized starts must remain required"],
  [evidence.fixture?.ignored_by_default === true, "tests must remain ignored by default"],
  [evidence.scenarios?.same_target_role_replacement?.expected_generation_delta === 2, "role replacement generation delta must remain two"],
  [evidence.scenarios?.last_active_super_admin?.expected_successes === 1, "exactly one super-admin demotion must succeed"],
  [evidence.scenarios?.last_active_super_admin?.expected_rejections === 1, "exactly one super-admin demotion must be rejected"],
  [evidence.scenarios?.generation_allocation?.concurrent_transactions === 8, "generation allocation must retain eight transactions"],
  [evidence.scenarios?.generation_allocation?.expected_unique === true, "unique generation requirement must remain true"],
  [evidence.scenarios?.generation_allocation?.expected_contiguous === true, "contiguous generation requirement must remain true"],
  [evidence.validation?.rust_test_executed === false, "source-shape JSON must not claim Rust execution"],
  [evidence.validation?.source_verifier_executed === false, "source-shape JSON must not claim verifier execution"],
  [evidence.validation?.postgresql_runtime_executed === false, "source-shape JSON must not claim PostgreSQL execution"],
  [evidence.multi_replica_evidence === false, "multi-replica evidence must remain open"],
  [evidence.redis_transport_evidence === false, "Redis evidence must remain open"],
  [evidence.cli_repair_live_replica_evidence === false, "CLI repair live-replica evidence must remain open"],
  [evidence.broad_rbac_verification_complete === false, "broad RBAC verification must remain open"],
  [evidence.cursor_advanced === false, "cursor must not advance"],
];
for (const [passed, message] of evidenceChecks) {
  if (!passed) failures.push(`${files.evidence}: ${message}`);
}

for (const marker of [
  "dedicated source-ready PostgreSQL harness",
  "Concurrent replacement of one user role",
  "Last-active-super-admin serialization",
  "Unique monotonic generation allocation",
  "There is no SQLite fallback.",
  "top-level harness cases must run serially at the libtest layer",
  "internal synchronized concurrency of two, two and eight operations",
  "--test-threads=1",
  "## Retained execution",
  "Runtime execution is retained by the workflow run and artifact above.",
  "does not prove Redis delivery",
]) requireNormalizedText(sources.docs, marker, `${files.docs}: evidence contract`);

for (const marker of [
  "### P0 — runtime evidence",
  "[x] Execute #2849 PostgreSQL concurrency",
  "[x] Execute #2853 independent-process watchdog recovery",
  "Status: `in_progress`",
]) requireNormalizedText(sources.plan, marker, `${files.plan}: owner gate`);

for (const marker of [
  "Current item: `core/rbac`",
  "Next item: `core/rbac`",
  "PostgreSQL concurrency packet #2849 passed 3/3",
  "durable watchdog packet #2853 passed 1/1",
]) requireNormalizedText(sources.master, marker, `${files.master}: active cursor`);

if (failures.length > 0) {
  console.error("RBAC PostgreSQL concurrency source verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ PostgreSQL RBAC concurrency source shape remains strict while the owner handoff retains completed #2849/#2853 runtime evidence and keeps later gates open",
);
