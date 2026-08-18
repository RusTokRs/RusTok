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
  cargo: "crates/rustok-cli/Cargo.toml",
  harness: "crates/rustok-cli/tests/rbac_live_repair_propagation.rs",
  cliRunner: "crates/rustok-cli/src/lib.rs",
  registry: "crates/rustok-cli-registry/src/generated.rs",
  rbacCli: "crates/rustok-rbac/cli/src/lib.rs",
  listener: "apps/server/src/services/rbac_cache_invalidation.rs",
  watchdog: "apps/server/src/services/rbac_invalidation_generation.rs",
  evidence:
    "crates/rustok-rbac/contracts/evidence/rbac-cli-live-repair-propagation-source.json",
  docs: "crates/rustok-rbac/docs/cli-live-repair-propagation-evidence.md",
  plan: "crates/rustok-rbac/docs/implementation-plan.md",
  master: "docs/verification/PLATFORM_VERIFICATION_PLAN.md",
};
const sources = Object.fromEntries(
  Object.entries(files).map(([name, relativePath]) => [name, read(relativePath)]),
);

for (const marker of [
  'rustok-server = { path = "../../apps/server", default-features = false }',
  "rustok-migrations.workspace = true",
  "rustok-telemetry.workspace = true",
  "rustok-test-utils.workspace = true",
]) requireText(sources.cargo, marker, `${files.cargo}: source harness dependencies`);
for (const forbidden of ["redis =", "rustok-rbac-cli"])
  forbidText(sources.cargo, forbidden, `${files.cargo}: isolated test dependency boundary`);

for (const marker of [
  '#[ignore = "requires PostgreSQL admin access and subprocess execution"]',
  "live_cli_system_role_repair_reaches_two_running_replicas_without_restart",
  'const CHILD_TEST_NAME: &str = "rbac_cli_live_repair_child"',
  'const WATCHDOG_RECOVERY_BOUND: Duration = Duration::from_secs(7)',
  "std::env::current_exe()",
  'child_command("observer"',
  'child_command("cli"',
  "start_rbac_cache_invalidation_listener",
  "start_rbac_invalidation_generation_watchdog",
  "RbacCacheInvalidationListenerHandle",
  "RbacInvalidationGenerationWatchdogHandle",
  "RBAC_INVALIDATION_APPLIED_GENERATION",
  'with_label_values(&["generation_advanced"])',
  "rustok_cli::run_with_runtime",
  '"repair-system-roles".to_string()',
  '"--apply".to_string()',
  '"--tenant-id".to_string()',
  "RuntimeComposition::from_database",
  "RbacRoleAssignmentDbWriter::new",
  "UserRole::Manager",
  "Permission::SETTINGS_MANAGE",
  "role_permission_links_removed",
  "affected_users_count != 2",
  "durable_generation != Some(1)",
  "runtime_restart_required_if_applied",
  '.env_remove("RUSTOK_REDIS_URL")',
  '.env_remove("REDIS_URL")',
  "cache.redis_configuration_present()",
  "listener.is_running()",
  "watchdog.is_running()",
  "process_id",
]) requireText(sources.harness, marker, `${files.harness}: live CLI repair topology`);

for (const forbidden of [
  "apply_system_role_repair_in_transaction",
  "reserve_permission_invalidation_generation",
  "repair_system_roles_committed",
  "invalidate_all_user_permissions_cache",
  "invalidate_user_permissions_cache",
  "invalidate_user_rbac_caches",
  "UPDATE rbac_invalidation_state",
  "SET generation = generation + 1",
  "publish_all_rbac_invalidation",
  "publish_user_rbac_invalidation",
  "VersionedCacheInvalidation::new",
  "CacheInvalidationMessage::new",
  "setup_test_db_with_migrations",
  "sqlite:",
  "redis::",
  "RUSTOK_CACHE_REDIS_SERVER_BIN",
  "RBAC_PERMISSION_INVALIDATION_CHANNEL",
]) forbidText(sources.harness, forbidden, `${files.harness}: forbidden shortcut`);

for (const marker of [
  "pub async fn run_with_runtime",
  "rustok_cli_registry::selected_distribution_registry(&runtime)",
  "registry.execute(request).await",
]) requireText(sources.cliRunner, marker, `${files.cliRunner}: canonical CLI runner`);

requireText(
  sources.registry,
  "rustok_rbac_cli::command_provider(runtime)",
  `${files.registry}: generated RBAC provider registration`,
);

for (const marker of [
  '"repair-system-roles"',
  "self.repair_system_roles(request.args).await",
  "apply_repair_with_generation(&db, tenant_id).await?",
  "apply_system_role_repair_in_transaction(&tx, tenant_id)",
  "reserve_permission_invalidation_generation(&tx)",
  "tx.commit().await",
  '"runtime_restart_required_if_applied"',
  '"durable_generation"',
]) requireText(sources.rbacCli, marker, `${files.rbacCli}: owner CLI repair path`);
for (const forbidden of [
  "publish_all_rbac_invalidation",
  "publish_user_rbac_invalidation",
  "CacheService",
]) forbidText(sources.rbacCli, forbidden, `${files.rbacCli}: CLI has no fan-out handle`);

for (const marker of [
  "listener.recover_generation_and_clear().await",
  "invalidate_all_user_permissions_cache().await",
  "ctx.shared_insert(runtime)",
]) requireText(sources.listener, marker, `${files.listener}: production listener baseline`);

for (const marker of [
  "const RBAC_DURABLE_GENERATION_RECONCILE_INTERVAL: Duration = Duration::from_secs(5)",
  '"generation_advanced"',
  "invalidate_all_user_permissions_cache().await",
  "state.observe_applied(generation)",
]) requireText(sources.watchdog, marker, `${files.watchdog}: durable recovery path`);

const evidence = JSON.parse(sources.evidence);
const evidenceChecks = [
  [evidence.status === "source_ready_unvalidated", "status must remain source_ready_unvalidated"],
  [evidence.base_revision === "c8cb49558f84c86dcd9e74d50466c198ddccecc8", "base revision must remain exact"],
  [evidence.cycle === "cycle-001", "cycle must remain cycle-001"],
  [evidence.component === "core/rbac", "component must remain core/rbac"],
  [evidence.severity === "P0", "severity must remain P0"],
  [evidence.topology?.database === "isolated_postgresql_database", "database must remain PostgreSQL"],
  [evidence.topology?.observer_processes === 2, "two observers are required"],
  [evidence.topology?.cli_processes === 1, "one CLI process is required"],
  [evidence.topology?.independent_os_processes === true, "OS process isolation is required"],
  [evidence.topology?.shared_process_cache === false, "process cache must not be shared"],
  [evidence.topology?.redis_configured === false, "Redis must remain disabled"],
  [evidence.topology?.observer_restart_allowed === false, "observer restart must remain forbidden"],
  [evidence.cli_path?.runner === "rustok_cli::run_with_runtime", "canonical CLI runner is required"],
  [evidence.cli_path?.runtime_cache_handle_present === false, "CLI cache handle must remain absent"],
  [evidence.cli_path?.runtime_redis_handle_present === false, "CLI Redis handle must remain absent"],
  [evidence.cli_path?.expected_durable_generation === 1, "durable generation must remain one"],
  [evidence.replica_recovery?.only_cross_process_recovery_path === "database_generation_watchdog", "watchdog must remain the only recovery path"],
  [evidence.replica_recovery?.recovery_reason === "generation_advanced", "generation advance recovery is required"],
  [evidence.replica_recovery?.full_clear_required_per_replica === true, "each replica must full-clear"],
  [evidence.replica_recovery?.maximum_recovery_ms === 7000, "recovery bound must remain 7000 ms"],
  [evidence.validation?.rust_test_executed === false, "Rust execution must not be claimed"],
  [evidence.validation?.source_verifier_executed === false, "verifier execution must not be claimed"],
  [evidence.validation?.postgresql_runtime_executed === false, "PostgreSQL execution must not be claimed"],
  [evidence.validation?.redis_runtime_executed === false, "Redis execution must not be claimed"],
  [evidence.validation?.subprocess_runtime_executed === false, "subprocess execution must not be claimed"],
  [evidence.remaining_gates?.same_revision_compile_and_module_gates === false, "compile gates must remain open"],
  [evidence.remaining_gates?.live_negative_transport_requests === false, "transport gate must remain open"],
  [evidence.remaining_gates?.core_rbac_complete === false, "RBAC must remain incomplete"],
  [evidence.cursor_advanced === false, "cursor must not advance"],
];
for (const [passed, message] of evidenceChecks) {
  if (!passed) failures.push(`${files.evidence}: ${message}`);
}

for (const marker of [
  "two independent long-lived observer processes",
  "one independent short-lived CLI process",
  "Redis configuration is explicitly removed",
  "database generation watchdog is the only cross-process recovery path",
  "rustok_cli::run_with_runtime",
  "does not receive a server cache service",
  "generation_advanced",
  "same process identifier before and after repair",
  "source_ready_unvalidated",
  "live negative HTTP, GraphQL, WebSocket and native transport requests",
]) requireNormalizedText(sources.docs, marker, `${files.docs}: evidence boundary`);

for (const marker of [
  "### P0 — runtime evidence",
  "Execute #2856 Redis available/outage/restart recovery — PR #3579 run",
  "Execute #2862 registered-CLI repair propagation — PR #3590 exact-head run",
  "Retain one same-revision result set within documented bounds",
  "Status: `in_progress`",
]) requireNormalizedText(sources.plan, marker, `${files.plan}: owner gate`);

for (const marker of [
  "Current item: `core/rbac`",
  "Next item: `core/rbac`",
  "Release readiness: `not_assessed`",
]) requireNormalizedText(sources.master, marker, `${files.master}: active cursor`);

if (failures.length > 0) {
  console.error("RBAC CLI live repair propagation source verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ source-ready RBAC CLI repair harness remains strict while the owner handoff retains successful live registered-CLI propagation and keeps the broader component gates open",
);
