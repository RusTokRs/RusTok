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
  packet: "apps/server/tests/rbac_policy_incident_trace.rs",
  service: "apps/server/src/services/rbac_service.rs",
  authoritative: "apps/server/src/services/rbac_authoritative.rs",
  runtime: "apps/server/src/services/rbac_runtime.rs",
  generation: "apps/server/src/services/rbac_invalidation_generation.rs",
  metrics: "crates/rustok-telemetry/src/rbac_invalidation_metrics.rs",
  evidence:
    "crates/rustok-rbac/contracts/evidence/rbac-policy-incident-trace-source.json",
  docs: "crates/rustok-rbac/docs/policy-incident-trace.md",
  plan: "crates/rustok-rbac/docs/implementation-plan.md",
  master: "docs/verification/PLATFORM_VERIFICATION_PLAN.md",
};

const sources = Object.fromEntries(
  Object.entries(files).map(([name, relativePath]) => [name, read(relativePath)]),
);

for (const marker of [
  "struct RbacPolicyIncidentPacket",
  "missed_publication_incident_connects_decision_relations_cache_generation_and_recovery",
  "RbacRoleAssignmentDbWriter::new(db.clone())",
  "RbacService::has_permission",
  "RbacService::get_user_permissions_authoritative",
  "RbacService::get_user_permissions",
  "reserve_permission_invalidation_generation(&transaction)",
  "transaction.commit()",
  "durable_generation > applied_generation_before",
  "permission_cache_hits + 1",
  "RBAC_INVALIDATION_RECOVERIES_TOTAL",
  "RBAC_INVALIDATION_FULL_CLEARS_TOTAL",
  'recovery_action: "generation_advanced_full_clear"',
  '"rbac policy incident packet"',
  "assert!(!packet.evaluator_allowed_after_recovery)",
]) requireText(sources.packet, marker, `${files.packet}: packet scenario`);

for (const forbidden of [
  "publish_user_rbac_invalidation(",
  "publish_all_rbac_invalidation(",
  "invalidate_all_user_permissions_cache(",
  "invalidate_user_permissions_cache(",
  "password_hash =",
  "access_token =",
  "bearer =",
]) forbidText(sources.packet, forbidden, `${files.packet}: injected shortcut or secret`);

for (const marker of [
  "rbac resolver decision (single permission check)",
  "permissions_count",
  "cache_hit",
  "allowed",
]) requireText(sources.service, marker, `${files.service}: canonical evaluator`);

for (const marker of [
  "get_user_permissions_authoritative",
  "user_belongs_to_tenant",
  "user_roles::Entity::find()",
  "role_permissions::Entity::find()",
  "permissions::Entity::find()",
]) requireText(sources.authoritative, marker, `${files.authoritative}: relation truth`);

for (const marker of [
  "CachedPermissionSnapshot",
  "PermissionCacheLookup",
  "permission_cache_hits",
  "invalidate_all_user_permissions_cache",
]) requireText(sources.runtime, marker, `${files.runtime}: cache snapshot`);

for (const marker of [
  "read_rbac_invalidation_generation",
  '"generation_advanced"',
  "invalidate_all_user_permissions_cache().await",
  "state.observe_applied(generation)",
]) requireText(sources.generation, marker, `${files.generation}: durable recovery`);

for (const marker of [
  "RBAC_INVALIDATION_DURABLE_GENERATION",
  "RBAC_INVALIDATION_APPLIED_GENERATION",
  "RBAC_INVALIDATION_RECOVERIES_TOTAL",
  "RBAC_INVALIDATION_FULL_CLEARS_TOTAL",
]) requireText(sources.metrics, marker, `${files.metrics}: bounded metrics`);

const evidence = JSON.parse(sources.evidence);
const evidenceChecks = [
  [evidence.status === "source_ready_unvalidated", "status must remain source_ready_unvalidated"],
  [evidence.cycle === "cycle-001", "cycle must remain cycle-001"],
  [evidence.component === "core/rbac", "component must remain core/rbac"],
  [evidence.failure_injection?.publisher_intentionally_not_called === true, "publisher omission must be explicit"],
  [evidence.failure_injection?.watchdog_is_the_recovery_actor === true, "watchdog must remain the recovery actor"],
  [evidence.failure_injection?.manual_test_only_cache_clear === false, "manual cache clear must remain forbidden"],
  [evidence.packet_contract?.raw_permission_list_logged === false, "raw permission lists must not be logged"],
  [evidence.validation?.rust_test_executed === false, "Rust execution must not be claimed"],
  [evidence.validation?.source_verifier_executed === false, "source verifier execution must not be claimed"],
  [evidence.broad_rbac_verification_complete === false, "broad RBAC verification must remain open"],
  [evidence.cursor_advanced === false, "cursor must not advance"],
];
for (const [passed, message] of evidenceChecks) {
  if (!passed) failures.push(`${files.evidence}: ${message}`);
}

for (const marker of [
  "dedicated integration scenario",
  "missed post-commit publication",
  "generation_advanced_full_clear",
  "The only recovery actor is the existing durable-generation watchdog.",
  "do not claim that PostgreSQL, Rust, Node, formatting, workflows, or CI have run",
  "core/rbac` cursor remains",
]) requireText(sources.docs, marker, `${files.docs}: incident contract`);

for (const marker of [
  "### P1. Invalidation observability and incident operations",
  "policy incident",
  "rbac_policy_incident_trace",
  "source_ready_unvalidated",
  "Status: `in_progress`",
]) requireText(sources.plan, marker, `${files.plan}: owner handoff`);

for (const marker of [
  "Current item: `core/rbac`",
  "Next item: `core/rbac`",
  "policy incident",
]) requireText(sources.master, marker, `${files.master}: active cursor`);

if (failures.length > 0) {
  console.error("RBAC policy incident trace verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ one source-ready RBAC policy incident packet connects evaluator, relation truth, stale cache, durable generation, watchdog recovery, and final denial",
);
