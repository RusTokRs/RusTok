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
  harness: "apps/server/tests/rbac_two_process_redis_restart.rs",
  invalidation: "apps/server/src/services/rbac_cache_invalidation.rs",
  mutation: "apps/server/src/services/rbac_committed_mutations.rs",
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
  "separate_process_redis_fast_path_survives_restart_and_recovers_missed_publication",
  'const CHILD_TEST_NAME: &str = "rbac_redis_replica_child"',
  'const FAST_PATH_BOUND: Duration = Duration::from_secs(3)',
  'const RESTART_RECOVERY_BOUND: Duration = Duration::from_secs(8)',
  'const REPLICA_SEQUENCE_BOUND: Duration = Duration::from_secs(25)',
  'const REDIS_SERVER_BIN_ENV: &str = "RUSTOK_CACHE_REDIS_SERVER_BIN"',
  'const CHILD_RESTART_ACK_PATH_ENV: &str = "RUSTOK_RBAC_REDIS_RESTART_ACK_PATH"',
  'let restart_ack_path = workspace.path().join("restart-result.ack")',
  ".env(CHILD_RESTART_ACK_PATH_ENV, restart_ack_path)",
  'std::fs::write(&restart_ack_path, b"release-observer")?',
  "wait_for_file(&restart_ack_path, Duration::from_secs(3)).await?",
  "std::env::current_exe()",
  'child_command("observer"',
  'child_command("mutator"',
  "wait_for_redis_subscribers(&redis_url, 1, \"parent initial observer subscription\")",
  '"observer child initial subscription"',
  '"observer resubscription after Redis restart"',
  "async fn wait_for_redis_subscribers(url: &str, expected: usize, stage: &str)",
  '"Redis did not expose {expected} RBAC subscribers during {stage}"',
  'redis::cmd("PUBSUB")',
  '.arg("NUMSUB")',
  "RBAC_PERMISSION_INVALIDATION_CHANNEL",
  "stop_redis(&mut redis_process)",
  "redis_process = spawn_redis",
  "replica_sequence_started.elapsed() > REPLICA_SEQUENCE_BOUND",
  "RbacRoleAssignmentDbWriter::new(db.clone())",
  "RbacService::replace_user_role_committed",
  "UserRole::Admin",
  "UserRole::Customer",
  "Permission::SETTINGS_MANAGE",
  "start_rbac_cache_invalidation_listener",
  "outage_result.allowed",
  "outage_result.authoritative_allowed",
  "restart_result.allowed",
]) requireText(sources.harness, marker, `${files.harness}: two-process Redis harness`);

for (const forbidden of [
  "start_rbac_invalidation_generation_watchdog",
  "invalidate_all_user_permissions_cache",
  "invalidate_user_permissions_cache",
  "invalidate_user_rbac_caches",
  "UPDATE rbac_invalidation_state",
  "SET generation = generation + 1",
  "publish_user_rbac_invalidation",
  "publish_all_rbac_invalidation",
  "setup_test_db_with_migrations",
  "sqlite:",
  "VersionedCacheInvalidation::new",
  "CacheInvalidationMessage::new",
]) forbidText(sources.harness, forbidden, `${files.harness}: forbidden shortcut`);

for (const marker of [
  'pub const RBAC_PERMISSION_INVALIDATION_CHANNEL: &str = "rbac.permissions.generation.v1"',
  "const RBAC_PERMISSION_RECONCILE_INTERVAL: Duration = Duration::from_secs(30)",
  "consume_subscription_with_ready",
  "ready_listener.recover_generation_and_clear().await",
  "invalidate_all_user_permissions_cache().await",
  "subscribe_local_channel",
]) requireText(sources.invalidation, marker, `${files.invalidation}: production recovery path`);

for (const marker of [
  "pub async fn replace_user_role_committed",
  "reserve_rbac_invalidation_generation(&tx)",
  "tx.commit().await?",
  "publish_user_rbac_invalidation(tenant_id, user_id, durable_generation)",
]) requireText(sources.mutation, marker, `${files.mutation}: committed mutation path`);

const evidence = JSON.parse(sources.evidence);
const evidenceChecks = [
  [evidence.status === "source_ready_unvalidated", "status must remain source_ready_unvalidated"],
  [evidence.cycle === "cycle-001", "cycle must remain cycle-001"],
  [evidence.component === "core/rbac", "component must remain core/rbac"],
  [evidence.severity === "P0", "severity must remain P0"],
  [evidence.topology?.database === "isolated_postgresql_database", "database must remain PostgreSQL"],
  [evidence.topology?.independent_os_processes === true, "OS process isolation is required"],
  [evidence.topology?.shared_process_cache === false, "process cache must not be shared"],
  [evidence.topology?.redis_server === "isolated_loopback_child_process", "isolated Redis is required"],
  [evidence.topology?.subscription_readiness_probe === "PUBSUB_NUMSUB", "NUMSUB readiness is required"],
  [evidence.fast_path_scenario?.observer_watchdog_started === false, "fast-path observer must not start watchdog"],
  [evidence.fast_path_scenario?.periodic_reconciliation_interval_ms === 30000, "listener poll must remain 30 seconds"],
  [evidence.fast_path_scenario?.maximum_decision_convergence_ms === 3000, "fast path bound must remain 3000 ms"],
  [evidence.restart_scenario?.mutation_committed_while_redis_unavailable === true, "outage commit is required"],
  [evidence.restart_scenario?.expected_outage_cached_decision === "allow", "outage cache must remain stale allow"],
  [evidence.restart_scenario?.expected_outage_authoritative_decision === "deny", "outage authority must be deny"],
  [evidence.restart_scenario?.same_observer_process_survives_restart === true, "observer continuity is required"],
  [evidence.restart_scenario?.maximum_reconnect_recovery_ms === 8000, "restart bound must remain 8000 ms"],
  [evidence.restart_scenario?.maximum_replica_sequence_ms === 25000, "sequence bound must remain 25000 ms"],
  [evidence.restart_scenario?.periodic_reconciliation_excluded === true, "periodic poll must be excluded"],
  [evidence.validation?.rust_test_executed === false, "Rust execution must not be claimed"],
  [evidence.validation?.source_verifier_executed === false, "verifier execution must not be claimed"],
  [evidence.validation?.postgresql_runtime_executed === false, "PostgreSQL execution must not be claimed"],
  [evidence.validation?.redis_runtime_executed === false, "Redis execution must not be claimed"],
  [evidence.validation?.subprocess_runtime_executed === false, "subprocess execution must not be claimed"],
  [evidence.remaining_gates?.cli_repair_live_replica_evidence === false, "CLI repair gate must remain open"],
  [evidence.remaining_gates?.full_multi_replica_gate_complete === false, "full gate must remain open"],
  [evidence.cursor_advanced === false, "cursor must not advance"],
];
for (const [passed, message] of evidenceChecks) {
  if (!passed) failures.push(`${files.evidence}: ${message}`);
}

for (const marker of [
  "one long-lived observer process",
  "PUBSUB NUMSUB",
  "does not start the durable-generation watchdog",
  "only available cross-process invalidation path inside this bound",
  "stops the isolated Redis process",
  "existing observer process must reconnect",
  "finish within twenty-five seconds",
  "cannot be attributed to the database poll fallback",
  "source_ready_unvalidated",
  "live CLI system-role repair propagation",
  "full multi-replica P0 gate remains open",
]) requireNormalizedText(sources.docs, marker, `${files.docs}: evidence boundary`);

for (const marker of [
  "### P0 — runtime evidence",
  "Execute #2856 Redis available/outage/restart recovery.",
  "Execute #2862 registered-CLI repair propagation.",
  "Status: `in_progress`",
]) requireNormalizedText(sources.plan, marker, `${files.plan}: owner gate`);

for (const marker of [
  "Current item: `core/rbac`",
  "Next item: `core/rbac`",
  "Redis available/outage/restart packet #2856",
  "CLI repair propagation #2862",
]) requireNormalizedText(sources.master, marker, `${files.master}: active cursor`);

if (failures.length > 0) {
  console.error("RBAC two-process Redis restart source verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ source-ready RBAC two-process Redis harness holds the recovered observer through parent NUMSUB proof, labels subscription phases, and retains the Redis/CLI runtime gates",
);
