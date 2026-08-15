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
  harness: "apps/server/tests/rbac_two_process_durable_recovery.rs",
  cacheRuntime: "apps/server/src/services/rbac_runtime.rs",
  invalidation: "apps/server/src/services/rbac_cache_invalidation.rs",
  watchdog: "apps/server/src/services/rbac_invalidation_generation.rs",
  mutation: "apps/server/src/services/rbac_committed_mutations.rs",
  evidence:
    "crates/rustok-rbac/contracts/evidence/rbac-two-process-durable-recovery-source.json",
  docs: "crates/rustok-rbac/docs/two-process-durable-recovery-evidence.md",
  plan: "crates/rustok-rbac/docs/implementation-plan.md",
  master: "docs/verification/PLATFORM_VERIFICATION_PLAN.md",
};
const sources = Object.fromEntries(
  Object.entries(files).map(([name, relativePath]) => [name, read(relativePath)]),
);

for (const marker of [
  '#[ignore = "requires PostgreSQL admin access and subprocess execution"]',
  "separate_process_replica_recovers_missed_local_publication_from_durable_generation",
  "std::env::current_exe()",
  "let observer = spawn_child(",
  "let mutator = spawn_child(",
  'const CHILD_TEST_NAME: &str = "rbac_multi_replica_child"',
  "RbacRoleAssignmentDbWriter::new(db.clone())",
  "UserRole::Admin",
  "Permission::SETTINGS_MANAGE",
  "start_rbac_cache_invalidation_listener",
  "start_rbac_invalidation_generation_watchdog",
  "RbacService::replace_user_role_committed",
  "read_permission_invalidation_generation",
  "allowed_after_commit_before_recovery",
  "DOCUMENTED_RECOVERY_BOUND",
  "Duration::from_secs(7)",
  "cache.redis_configuration_present()",
  "if mutation.redis_configured || observation.redis_configured",
  "authoritative.contains(&Permission::SETTINGS_MANAGE)",
]) requireText(sources.harness, marker, `${files.harness}: two-process harness`);

for (const forbidden of [
  "settings.cache.redis_url",
  "RUSTOK_CACHE_REAL_REDIS_URL",
  "invalidate_all_user_permissions_cache",
  "invalidate_user_permissions_cache",
  "invalidate_user_rbac_caches",
  "UPDATE rbac_invalidation_state",
  "SET generation = generation + 1",
  "publish_user_rbac_invalidation",
  "publish_all_rbac_invalidation",
  "setup_test_db_with_migrations",
  "sqlite:",
]) forbidText(sources.harness, forbidden, `${files.harness}: forbidden shortcut`);

for (const marker of [
  "static USER_PERMISSION_CACHE",
  "invalidate_all_user_permissions_cache",
  "MokaPermissionCache",
]) requireText(sources.cacheRuntime, marker, `${files.cacheRuntime}: process cache authority`);

for (const marker of [
  "RBAC_PERMISSION_INVALIDATION_CHANNEL",
  "subscribe_local_channel",
  "publish_durable",
  "start_rbac_cache_invalidation_listener",
]) requireText(sources.invalidation, marker, `${files.invalidation}: production fast path`);

for (const marker of [
  "RBAC_DURABLE_GENERATION_RECONCILE_INTERVAL",
  "Duration::from_secs(5)",
  "read_rbac_invalidation_generation(&db)",
  "invalidate_all_user_permissions_cache().await",
  "start_rbac_invalidation_generation_watchdog",
]) requireText(sources.watchdog, marker, `${files.watchdog}: durable recovery path`);

for (const marker of [
  "pub async fn replace_user_role_committed",
  "reserve_rbac_invalidation_generation(&tx)",
  "tx.commit().await?",
  "publish_user_rbac_invalidation(tenant_id, user_id, durable_generation)",
]) requireText(sources.mutation, marker, `${files.mutation}: committed mutation path`);

const evidence = JSON.parse(sources.evidence);
const evidenceChecks = [
  [evidence.status === "source_ready_unvalidated", "source-shape status must remain source_ready_unvalidated"],
  [evidence.cycle === "cycle-001", "cycle must remain cycle-001"],
  [evidence.component === "core/rbac", "component must remain core/rbac"],
  [evidence.topology?.database === "isolated_postgresql_database", "database must remain isolated PostgreSQL"],
  [evidence.topology?.observer_processes === 1, "one observer process is required"],
  [evidence.topology?.mutator_processes === 1, "one mutator process is required"],
  [evidence.topology?.independent_os_processes === true, "OS process isolation is required"],
  [evidence.topology?.shared_process_cache === false, "process cache must not be shared"],
  [evidence.topology?.shared_local_invalidation_bus === false, "local invalidation bus must not be shared"],
  [evidence.topology?.redis_configured === false, "Redis must remain disabled in this scenario"],
  [evidence.scenario?.expected_durable_generation_delta === 1, "generation delta must remain one"],
  [evidence.scenario?.expected_pre_recovery_decision === "allow_from_stale_process_cache", "stale pre-recovery allow is required"],
  [evidence.scenario?.expected_post_recovery_decision === "deny", "post-recovery deny is required"],
  [evidence.scenario?.maximum_recovery_elapsed_ms === 7000, "recovery bound must remain 7000 ms"],
  [evidence.validation?.rust_test_executed === false, "source-shape JSON must not claim Rust execution"],
  [evidence.validation?.source_verifier_executed === false, "source-shape JSON must not claim verifier execution"],
  [evidence.validation?.postgresql_runtime_executed === false, "source-shape JSON must not claim PostgreSQL execution"],
  [evidence.validation?.subprocess_runtime_executed === false, "source-shape JSON must not claim subprocess execution"],
  [evidence.redis_available_evidence === false, "source-shape JSON must remain immutable source-only evidence"],
  [evidence.redis_restart_evidence === false, "source-shape JSON must remain immutable source-only evidence"],
  [evidence.cli_repair_live_replica_evidence === false, "CLI repair source packet must remain open"],
  [evidence.full_multi_replica_gate_complete === false, "full multi-replica gate must remain open"],
  [evidence.cursor_advanced === false, "cursor must not advance"],
];
for (const [passed, message] of evidenceChecks) {
  if (!passed) failures.push(`${files.evidence}: ${message}`);
}

for (const marker of [
  "two independent operating-system processes",
  "intentionally does not configure Redis",
  "cannot receive the fast-path publication",
  "canonical five-second durable-generation watchdog",
  "source_ready_unvalidated",
  "## Retained execution",
  "Redis publication between live replicas",
  "The full multi-replica P0 gate remains open.",
]) requireNormalizedText(sources.docs, marker, `${files.docs}: evidence boundary`);

for (const marker of [
  "### P0 — runtime evidence",
  "Execute #2853 independent-process watchdog recovery",
  "Execute #2856 Redis available/outage/restart recovery",
  "Execute #2862 registered-CLI repair propagation",
  "Status: `in_progress`",
]) requireNormalizedText(sources.plan, marker, `${files.plan}: owner gate`);

for (const marker of [
  "Current item: `core/rbac`",
  "Next item: `core/rbac`",
  "durable watchdog packet #2853 passed 1/1",
  "Redis available/outage/restart packet #2856",
]) requireNormalizedText(sources.master, marker, `${files.master}: active cursor`);

if (failures.length > 0) {
  console.error("RBAC two-process durable recovery source verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ two-process durable-recovery source shape remains strict while the owner handoff retains completed #2853/#2856 evidence and keeps the CLI gate open",
);
