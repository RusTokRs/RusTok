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
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const files = {
  harness: "apps/server/tests/rbac_two_process_redis_restart_recovery.rs",
  cacheRuntime: "apps/server/src/services/cache_runtime.rs",
  invalidation: "apps/server/src/services/rbac_cache_invalidation.rs",
  mutation: "apps/server/src/services/rbac_committed_mutations.rs",
  cacheService: "crates/rustok-cache/src/service.rs",
  evidence:
    "crates/rustok-rbac/contracts/evidence/rbac-two-process-redis-restart-source.json",
  docs: "crates/rustok-rbac/docs/two-process-redis-restart-evidence.md",
  plan: "crates/rustok-rbac/docs/implementation-plan.md",
  master: "docs/verification/PLATFORM_VERIFICATION_PLAN.md",
};
const sources = Object.fromEntries(
  Object.entries(files).map(([name, relativePath]) => [name, read(relativePath)]),
);

for (const marker of [
  '#[ignore = "requires PostgreSQL admin access, redis-server and subprocess execution"]',
  "separate_process_replica_applies_available_redis_invalidation",
  "separate_process_replica_recovers_after_redis_restart_and_resubscribe",
  'const CHILD_TEST_NAME: &str = "rbac_redis_mutator_child"',
  "std::env::current_exe()",
  "RUSTOK_CACHE_REDIS_SERVER_BIN",
  "RedisCommand::new(binary)",
  'redis::cmd("PUBSUB")',
  '.arg("NUMSUB")',
  "start_rbac_cache_invalidation_listener",
  "RbacService::replace_user_role_committed",
  "RbacRoleAssignmentDbWriter::new(db.clone())",
  "Permission::SETTINGS_MANAGE",
  "redis_publish_success_total",
  "redis_publish_failure_total",
  "FAST_PATH_BOUND",
  "RESTART_RECOVERY_BOUND",
  "UserRole::Admin",
  "UserRole::Customer",
  "stale_allowed",
  "wait_for_terminal_deny",
  "get_user_permissions_authoritative",
  "#[serial_test::serial]",
]) requireText(sources.harness, marker, `${files.harness}: two-process Redis harness`);

for (const forbidden of [
  "start_rbac_invalidation_generation_watchdog",
  "invalidate_all_user_permissions_cache",
  "invalidate_user_permissions_cache",
  "invalidate_user_rbac_caches",
  "reserve_permission_invalidation_generation",
  "UPDATE rbac_invalidation_state",
  "SET generation = generation + 1",
  "publish_user_rbac_invalidation",
  "publish_all_rbac_invalidation",
  "VersionedCacheInvalidation::new",
  "DurableCacheInvalidationRecord::new",
  "setup_test_db_with_migrations",
  "sqlite:",
]) forbidText(sources.harness, forbidden, `${files.harness}: forbidden shortcut`);

for (const marker of [
  "CacheService::from_url(ctx.settings().cache.redis_url.as_deref())",
  "Return the single process-wide cache service",
]) requireText(sources.cacheRuntime, marker, `${files.cacheRuntime}: production cache owner`);

for (const marker of [
  "RBAC_PERMISSION_INVALIDATION_CHANNEL",
  "publish_durable(&record)",
  "consume_subscription_with_ready",
  "ready_listener.recover_generation_and_clear().await",
  "spawn_supervised_rbac_invalidation_worker",
  '"redis_worker_restart"',
  "start_rbac_cache_invalidation_listener",
]) requireText(sources.invalidation, marker, `${files.invalidation}: production Redis lifecycle`);

for (const marker of [
  "pub async fn replace_user_role_committed",
  "reserve_rbac_invalidation_generation(&tx)",
  "tx.commit().await?",
  "publish_user_rbac_invalidation(tenant_id, user_id, durable_generation)",
]) requireText(sources.mutation, marker, `${files.mutation}: committed mutation path`);

for (const marker of [
  "consume_subscription_with_ready",
  'client.get_async_pubsub()',
  "pubsub.subscribe(channel)",
  "ready().await",
  'redis::cmd("PUBLISH")',
  "redis_publish_success_total",
  "redis_publish_failure_total",
]) requireText(sources.cacheService, marker, `${files.cacheService}: canonical Redis transport`);

const evidence = JSON.parse(sources.evidence);
const evidenceChecks = [
  [evidence.status === "source_ready_unvalidated", "status must remain source_ready_unvalidated"],
  [evidence.cycle === "cycle-001", "cycle must remain cycle-001"],
  [evidence.component === "core/rbac", "component must remain core/rbac"],
  [evidence.topology?.database === "isolated_postgresql_database", "database must remain isolated PostgreSQL"],
  [evidence.topology?.observer_processes === 1, "one observer process is required"],
  [evidence.topology?.mutator_processes === 1, "one mutator process is required"],
  [evidence.topology?.independent_os_processes === true, "OS process isolation is required"],
  [evidence.topology?.shared_process_cache === false, "process cache must not be shared"],
  [evidence.topology?.shared_local_invalidation_bus === false, "local invalidation bus must not be shared"],
  [evidence.topology?.redis_server === "isolated_spawned_redis_server", "an isolated spawned Redis server is required"],
  [evidence.available_scenario?.expected_durable_generation_delta === 1, "available generation delta must remain one"],
  [evidence.available_scenario?.expected_redis_publish_success_minimum === 1, "available Redis publication success is required"],
  [evidence.available_scenario?.maximum_fast_path_elapsed_ms === 3000, "available fast-path bound must remain 3000 ms"],
  [evidence.restart_scenario?.redis_state_at_mutation === "stopped", "restart mutation must occur while Redis is stopped"],
  [evidence.restart_scenario?.expected_redis_publish_failure_minimum === 1, "restart scenario must retain failed publication evidence"],
  [evidence.restart_scenario?.expected_pre_restart_decision === "allow_from_stale_observer_cache", "a stale pre-restart allow is required"],
  [evidence.restart_scenario?.recovery_actor === "supervised Redis subscription ready callback", "subscriber-ready recovery must remain the recovery actor"],
  [evidence.restart_scenario?.maximum_restart_recovery_elapsed_ms === 5000, "restart recovery bound must remain 5000 ms"],
  [evidence.isolation_contract?.watchdog_disabled_in_harness === true, "watchdog isolation must remain explicit"],
  [evidence.validation?.rust_test_executed === false, "Rust execution must not be claimed"],
  [evidence.validation?.source_verifier_executed === false, "source verifier execution must not be claimed"],
  [evidence.validation?.postgresql_runtime_executed === false, "PostgreSQL execution must not be claimed"],
  [evidence.validation?.redis_runtime_executed === false, "Redis execution must not be claimed"],
  [evidence.validation?.subprocess_runtime_executed === false, "subprocess execution must not be claimed"],
  [evidence.redis_available_source_evidence === true, "available Redis source evidence must be recorded"],
  [evidence.redis_restart_source_evidence === true, "restart Redis source evidence must be recorded"],
  [evidence.runtime_evidence_retained === false, "runtime evidence must remain absent"],
  [evidence.cli_repair_live_replica_evidence === false, "CLI repair evidence must remain open"],
  [evidence.full_multi_replica_gate_complete === false, "full multi-replica gate must remain open"],
  [evidence.cursor_advanced === false, "cursor must not advance"],
];
for (const [passed, message] of evidenceChecks) {
  if (!passed) failures.push(`${files.evidence}: ${message}`);
}

for (const marker of [
  "two independent operating-system processes",
  "Available Redis scenario",
  "Redis restart scenario",
  "PUBSUB NUMSUB",
  "The harness intentionally does not start the database watchdog.",
  "Merged PR #2853 owns the separate watchdog-fallback source packet.",
  "source_ready_unvalidated",
  "It does not prove:",
  "the complete multi-replica P0 gate",
]) requireText(sources.docs, marker, `${files.docs}: evidence boundary`);

for (const marker of [
  "### P0. Database concurrency and multi-replica recovery evidence",
  "Redis available and restart/resubscribe source",
  "CLI system-role repair",
  "RBAC two-process Redis restart source packet",
  "Status: `in_progress`",
]) requireText(sources.plan, marker, `${files.plan}: owner gate`);

for (const marker of [
  "Current item: `core/rbac`",
  "Next item: `core/rbac`",
  "Draft #2857 adds the complementary Redis available",
  "CLI repair propagation",
]) requireText(sources.master, marker, `${files.master}: active cursor`);

if (failures.length > 0) {
  console.error("RBAC two-process Redis restart source verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ source-ready RBAC Redis harness isolates cross-process fast-path delivery and subscriber-ready durable recovery after restart without watchdog or manual invalidation shortcuts",
);