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
  harness: "crates/rustok-rbac/cli/tests/live_repair_replica_recovery.rs",
  cliCargo: "crates/rustok-rbac/cli/Cargo.toml",
  cliOwner: "crates/rustok-rbac/cli/src/lib.rs",
  watchdog: "apps/server/src/services/rbac_invalidation_generation.rs",
  evidence: "crates/rustok-rbac/contracts/evidence/rbac-cli-live-repair-source.json",
  docs: "crates/rustok-rbac/docs/cli-live-repair-replica-evidence.md",
  plan: "crates/rustok-rbac/docs/implementation-plan.md",
  master: "docs/verification/PLATFORM_VERIFICATION_PLAN.md",
};
const sources = Object.fromEntries(
  Object.entries(files).map(([name, relativePath]) => [name, read(relativePath)]),
);

for (const marker of [
  '#[ignore = "requires PostgreSQL admin access and subprocess execution"]',
  "cli_system_role_repair_recovers_two_live_replicas_without_restart",
  'const CHILD_TEST_NAME: &str = "rbac_cli_repair_live_replica_child"',
  'spawn_child(\n        "observer"',
  'spawn_child(\n        "cli"',
  "std::env::current_exe()",
  "rustok_rbac_cli::command_provider(&runtime)",
  "CommandRequest",
  'namespace: "rbac"',
  'name: "repair-system-roles"',
  '"apply": true',
  "RuntimeComposition::from_database",
  "start_rbac_invalidation_generation_watchdog",
  "RbacService::has_permission",
  "get_user_permissions_authoritative",
  "UserRole::Manager",
  "Permission::SETTINGS_MANAGE",
  "OBSERVER_RECOVERY_BOUND",
  "runtime_restart_required_if_applied",
  "durable_generation",
  "changes_total",
]) requireText(sources.harness, marker, `${files.harness}: live CLI repair topology`);

const observerSpawns = sources.harness.match(/spawn_child\(\s*"observer"/g)?.length ?? 0;
if (observerSpawns !== 2) {
  failures.push(`${files.harness}: expected exactly two observer process spawns, found ${observerSpawns}`);
}

for (const forbidden of [
  "apply_system_role_repair_in_transaction",
  "plan_system_role_repair",
  "reserve_permission_invalidation_generation",
  "invalidate_all_user_permissions_cache",
  "invalidate_user_permissions_cache",
  "invalidate_user_rbac_caches",
  "publish_user_rbac_invalidation",
  "publish_all_rbac_invalidation",
  "start_rbac_cache_invalidation_listener",
  "redis://",
  "sqlite:",
]) forbidText(sources.harness, forbidden, `${files.harness}: forbidden repair shortcut`);

for (const marker of [
  "rustok-server = { path = \"../../../apps/server\", default-features = false }",
  "rustok-migrations.workspace = true",
  "rustok-test-utils.workspace = true",
]) requireText(sources.cliCargo, marker, `${files.cliCargo}: bounded integration dependencies`);

for (const marker of [
  '("rbac", "repair-system-roles") => self.repair_system_roles(request.args).await',
  "apply_repair_with_generation(&db, tenant_id).await?",
  "apply_system_role_repair_in_transaction(&tx, tenant_id)",
  "reserve_permission_invalidation_generation(&tx)",
  "tx.commit().await.map_err(command_failed)?",
  '"runtime_restart_required_if_applied"',
  '"durable_generation"',
  '"RBAC system role repair applied with durable invalidation"',
]) requireText(sources.cliOwner, marker, `${files.cliOwner}: production CLI transaction contract`);

for (const marker of [
  "RBAC_DURABLE_GENERATION_RECONCILE_INTERVAL",
  "read_rbac_invalidation_generation(&db).await",
  '"generation_advanced"',
  "invalidate_all_user_permissions_cache().await",
  "start_rbac_invalidation_generation_watchdog",
]) requireText(sources.watchdog, marker, `${files.watchdog}: production replica recovery`);

const evidence = JSON.parse(sources.evidence);
const evidenceChecks = [
  [evidence.status === "source_ready_unvalidated", "status must remain source_ready_unvalidated"],
  [evidence.cycle === "cycle-001", "cycle must remain cycle-001"],
  [evidence.component === "core/rbac", "component must remain core/rbac"],
  [evidence.topology?.database === "isolated_postgresql_database", "database must remain isolated PostgreSQL"],
  [evidence.topology?.observer_processes === 2, "two observers are required"],
  [evidence.topology?.cli_processes === 1, "one CLI process is required"],
  [evidence.topology?.independent_os_processes === true, "OS process isolation is required"],
  [evidence.topology?.shared_process_cache === false, "observer caches must not be shared"],
  [evidence.topology?.redis_configured === false, "Redis must remain absent"],
  [evidence.topology?.replica_restart_permitted === false, "replica restart must remain forbidden"],
  [evidence.cli_contract?.provider === "rustok_rbac_cli::command_provider", "production provider is required"],
  [evidence.cli_contract?.command === "repair-system-roles", "repair-system-roles command is required"],
  [evidence.cli_contract?.apply === true, "apply mode is required"],
  [evidence.cli_contract?.expected_durable_generation === 1, "generation one is required"],
  [evidence.cli_contract?.runtime_restart_required_if_applied === false, "restart-required must remain false"],
  [evidence.recovery_contract?.expected_observer_count === 2, "two recovered observers are required"],
  [evidence.recovery_contract?.maximum_observer_recovery_elapsed_ms === 8000, "recovery bound must remain 8000 ms"],
  [evidence.validation?.rust_test_executed === false, "Rust execution must not be claimed"],
  [evidence.validation?.source_verifier_executed === false, "source verifier execution must not be claimed"],
  [evidence.validation?.postgresql_runtime_executed === false, "PostgreSQL execution must not be claimed"],
  [evidence.validation?.subprocess_runtime_executed === false, "subprocess execution must not be claimed"],
  [evidence.source_evidence_present === true, "source evidence must be present"],
  [evidence.runtime_evidence_retained === false, "runtime evidence must remain absent"],
  [evidence.full_multi_replica_gate_complete === false, "multi-replica gate must remain open"],
  [evidence.cursor_advanced === false, "cursor must not advance"],
];
for (const [passed, message] of evidenceChecks) {
  if (!passed) failures.push(`${files.evidence}: ${message}`);
}

for (const marker of [
  "two independent live observer processes",
  "one independent CLI process",
  "rustok_rbac_cli::command_provider",
  "without requiring a process restart",
  "Forbidden shortcuts",
  "source_ready_unvalidated",
  "the complete multi-replica P0 gate",
]) requireText(sources.docs, marker, `${files.docs}: source evidence boundary`);

for (const marker of [
  "live CLI system-role repair source evidence",
  "PR #2856",
  "Status: `in_progress`",
  "P0=1, P1=2",
]) requireText(sources.plan, marker, `${files.plan}: owner handoff`);

for (const marker of [
  "Current item: `core/rbac`",
  "Next item: `core/rbac`",
  "Draft #2859 adds the remaining live CLI",
  "No execution evidence is claimed.",
]) requireText(sources.master, marker, `${files.master}: active cursor`);

if (failures.length > 0) {
  console.error("RBAC live CLI repair source verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ source-ready RBAC CLI repair harness uses the production command provider and recovers two independent live replicas only through the committed durable generation",
);