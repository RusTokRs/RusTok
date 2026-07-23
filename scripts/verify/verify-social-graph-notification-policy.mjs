#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function requireOrder(source, markers, message) {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker, previous + 1);
    if (index < 0 || index <= previous) {
      failures.push(`${message}: missing or out-of-order marker ${marker}`);
      return;
    }
    previous = index;
  }
}

function reject(source, pattern, message) {
  if (pattern.test(source)) failures.push(message);
}

const contractPath =
  "crates/rustok-social-graph/contracts/social-graph-notification-policy.json";
const contract = JSON.parse(read(contractPath) || "{}");
const migration = read(contract.migration ?? "");
const entity = read(contract.entity ?? "");
const service = read(contract.service ?? "");
const ports = read(contract.ports ?? "");
const adapter = read(contract.server_composition?.adapter ?? "");
const runtime = read("crates/rustok-notifications/src/recipient_policy.rs");
const registry = read(contract.distribution?.registry ?? "");
const manifest = read(contract.distribution?.manifest ?? "");
const test = read("crates/rustok-social-graph/tests/privacy_sqlite.rs");

if (contract.slice !== "SOCIAL-01A/NOTIFY-07C") {
  failures.push("machine contract must identify SOCIAL-01A/NOTIFY-07C");
}
if (contract.privacy_semantics?.block_either_direction_suppresses !== true) {
  failures.push("block privacy must suppress in either direction");
}
if (contract.privacy_semantics?.mute_source_to_target_only !== true) {
  failures.push("mute privacy must remain directional");
}
if (contract.server_composition?.relation_ports_ready !== true) {
  failures.push("concrete social graph adapters must make relation ports ready");
}
if (contract.server_composition?.candidate_worker_enabled !== false) {
  failures.push("candidate worker must remain disabled in this slice");
}

for (const marker of [
  "CREATE TABLE IF NOT EXISTS social_graph_relations",
  "ux_social_graph_relation_identity",
  "FOREIGN KEY (tenant_id, source_user_id)",
  "FOREIGN KEY (tenant_id, target_user_id)",
  "source_user_id <> target_user_id",
  "relation_kind IN ('block', 'mute')",
  "revision > 0",
]) {
  requireText(migration, marker, `social graph migration is missing ${marker}`);
}

for (const marker of [
  'table_name = "social_graph_relations"',
  "SocialRelationKind",
  "source_user_id",
  "target_user_id",
  "revision",
]) {
  requireText(entity, marker, `social graph entity is missing ${marker}`);
}

for (const marker of [
  "set_relation_state",
  "expected_revision",
  "rows_affected != 1",
  "Condition::any()",
  "SocialRelationKind::Block",
  "SocialRelationKind::Mute",
]) {
  requireText(service, marker, `social graph service is missing ${marker}`);
}
requireOrder(
  service,
  [
    "RelationKind.eq(SocialRelationKind::Block)",
    "Condition::any()",
  ],
  "block lookup must evaluate both relation directions",
);

for (const marker of [
  "pub trait SocialGraphCommandPort",
  "pub trait SocialGraphPrivacyReadPort",
  "context.require_policy(PortCallPolicy::write())",
  "context.require_policy(PortCallPolicy::read())",
  "validate_source_actor",
]) {
  requireText(ports, marker, `social graph owner ports are missing ${marker}`);
}

for (const marker of [
  "SocialGraphNotificationBlockAdapter",
  "SocialGraphNotificationMuteAdapter",
  "SocialGraphPrivacyReadPort",
  "SocialGraphService::new",
  "NotificationBlockReadRuntime::new",
  "NotificationMuteReadRuntime::new",
  "NotificationRecipientPolicyRuntime::new(Arc::new(policy), true)",
]) {
  requireText(adapter, marker, `server social graph adapter is missing ${marker}`);
}
requireOrder(
  adapter,
  [
    "evaluate_profile_privacy",
    "blocks_notification",
    "mutes_notification",
    "NotificationRecipientPolicyDecision::Allow",
  ],
  "recipient policy must evaluate profile, block, and mute state before allow",
);
reject(
  adapter,
  /social_graph_relations|relation::Entity|Column::SourceUserId|Column::TargetUserId/,
  "server adapter must call the Social Graph owner port instead of private tables",
);
reject(
  adapter,
  /AllowAll|Permissive|unwrap_or\(false\)|unwrap_or_default\(\)/,
  "server adapter must not convert missing or failed privacy state into allow",
);

for (const marker of [
  "relation_ports_ready: bool",
  "candidate_worker_enabled: bool",
  "with_candidate_worker_enabled",
  "self.relation_ports_ready && self.candidate_worker_enabled",
]) {
  requireText(runtime, marker, `notification runtime gate is missing ${marker}`);
}

requireText(
  registry,
  "registry.register(SocialGraphModule)",
  "compiled distribution must register SocialGraphModule",
);
requireText(
  manifest,
  'social_graph = { crate = "rustok-social-graph"',
  "module manifest must declare social_graph",
);

for (const marker of [
  "block_and_mute_state_is_tenant_scoped_and_replay_safe",
  "block privacy is strict in either direction",
  "mute remains directional",
  "tenant-composite foreign key must reject foreign users",
]) {
  requireText(test, marker, `SQLite owner scenario is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Social Graph notification policy verification failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Social Graph notification policy boundary verified.");
