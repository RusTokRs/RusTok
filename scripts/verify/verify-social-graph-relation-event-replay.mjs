#!/usr/bin/env node
// Social Graph bounded relation-event replay and reconciliation guardrails.

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function readRepo(relativePath) {
  return readFileSync(repoPath(relativePath), "utf8");
}

function fail(message) {
  failures.push(message);
}

function assertExists(relativePath) {
  if (!existsSync(repoPath(relativePath))) fail(`${relativePath}: expected file`);
}

function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) fail(description);
}

function assertNotContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (found) fail(description);
}

const paths = {
  event: "crates/rustok-events/src/social_graph.rs",
  ports: "crates/rustok-social-graph/src/ports.rs",
  maintenance: "crates/rustok-social-graph/src/maintenance.rs",
  lib: "crates/rustok-social-graph/src/lib.rs",
  readme: "crates/rustok-social-graph/README.md",
  plan: "crates/rustok-social-graph/docs/implementation-plan.md",
  profilesPlan: "crates/rustok-profiles/docs/implementation-plan.md",
  test: "crates/rustok-social-graph/tests/relation_event_replay_sqlite.rs",
};

for (const value of Object.values(paths)) assertExists(value);

const event = readRepo(paths.event);
const ports = readRepo(paths.ports);
const maintenance = readRepo(paths.maintenance);
const lib = readRepo(paths.lib);
const readme = readRepo(paths.readme);
const plan = readRepo(paths.plan);
const profilesPlan = readRepo(paths.profilesPlan);
const test = readRepo(paths.test);

assertContains(
  event,
  'description: "A tenant-scoped social relation state fact for one persisted revision."',
  `${paths.event}: event must be a replayable persisted-revision fact`,
);

for (const marker of [
  "MAX_SOCIAL_GRAPH_RELATION_EVENT_REPLAY_BATCH",
  "SocialGraphRelationEventReplayCommand",
  "after_relation_id: Option<Uuid>",
  "SocialGraphRelationEventReplayResult",
  "selected_relations: u64",
  "published_events: u64",
  "next_after_relation_id: Option<Uuid>",
  "trait SocialGraphRelationEventMaintenancePort",
  "replay_relation_state_events",
]) {
  assertContains(ports, marker, `${paths.ports}: replay port contract missing: ${marker}`);
}

for (const marker of [
  "SocialGraphRelationEventMaintenanceService",
  "event_bus: TransactionalEventBus",
  "PortCallPolicy::event_replay()",
  '"social_graph.relation_event_replay_forbidden"',
  '"social_graph.relation_event_replay_limit_invalid"',
  "relation::Column::TenantId.eq(tenant_id)",
  "order_by_asc(relation::Column::Id)",
  "relation::Column::Id.gt(after_relation_id)",
  ".limit(u64::from(command.limit))",
  "event_for_relation(relation)",
  "publish_contract_in_tx",
  "transaction.rollback().await",
  "transaction.commit().await",
  'operation = RELATION_EVENT_REPLAY_OPERATION',
  "selected_relations",
  "published_events",
  "cursor_present = command.after_relation_id.is_some()",
]) {
  assertContains(
    maintenance,
    marker,
    `${paths.maintenance}: replay implementation guardrail missing: ${marker}`,
  );
}

const publishIndex = maintenance.indexOf("publish_contract_in_tx");
const rollbackIndex = maintenance.indexOf("transaction.rollback().await", publishIndex);
const commitIndex = maintenance.indexOf("transaction.commit().await", publishIndex);
if (!(publishIndex >= 0 && rollbackIndex > publishIndex && commitIndex > publishIndex)) {
  fail(`${paths.maintenance}: expected publish with rollback and commit paths`);
}

assertContains(
  maintenance,
  "if !command.dry_run",
  `${paths.maintenance}: dry-run must not publish events`,
);
assertNotContains(
  maintenance,
  /after_relation_id\s*=\s*[%?]/,
  `${paths.maintenance}: raw replay cursor must not enter telemetry`,
);
assertNotContains(
  maintenance,
  /relation_id\s*=\s*%/,
  `${paths.maintenance}: per-relation ids must not enter aggregate replay telemetry`,
);

for (const marker of [
  "SocialGraphRelationEventMaintenanceService",
  "MAX_SOCIAL_GRAPH_RELATION_EVENT_REPLAY_BATCH",
  "SocialGraphRelationEventMaintenancePort",
  "SocialGraphRelationEventReplayCommand",
  "SocialGraphRelationEventReplayResult",
]) {
  assertContains(lib, marker, `${paths.lib}: public replay export missing: ${marker}`);
}

for (const marker of [
  "bounded_replay_is_tenant_scoped_cursor_driven_and_dry_run_safe",
  "replay_rejects_user_actor_and_invalid_limit_without_events",
  "replay_rolls_back_the_whole_batch_when_one_outbox_insert_fails",
  "fail_second_social_graph_replay_event",
  "assert_eq!(event_count(&db).await, 0)",
  "other_tenant_id",
  "next_after_relation_id",
  "created_at: Set(now.clone())",
]) {
  assertContains(test, marker, `${paths.test}: replay scenario missing: ${marker}`);
}

for (const marker of [
  "bounded relation-event replay",
  "service/system",
  "UUID cursor",
  "monotonic revision",
  "authoritative",
]) {
  assertContains(
    `${readme}\n${plan}\n${profilesPlan}`,
    marker,
    `relation replay documentation missing: ${marker}`,
  );
}

for (const forbidden of [
  "idempotency_key =",
  "expected_revision =",
  "request_json =",
  "response_json =",
  "claims =",
  "roles =",
  "locale =",
  "channel =",
]) {
  assertNotContains(
    maintenance,
    forbidden,
    `${paths.maintenance}: replay telemetry leaked forbidden field: ${forbidden}`,
  );
}

if (failures.length > 0) {
  console.error("Social Graph relation event replay verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Social Graph relation event replay verification passed");
