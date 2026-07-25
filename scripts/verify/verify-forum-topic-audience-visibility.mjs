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

function rejectText(source, marker, message) {
  if (source.includes(marker)) failures.push(message);
}

const contractPath =
  "crates/rustok-forum/contracts/forum-topic-audience-visibility.json";
const contract = JSON.parse(read(contractPath) || "{}");
const owner = read(contract.owner_file ?? "");
const baseOwner = read(contract.base_visibility_owner ?? "");
const categoryOwner = read(contract.category_policy_owner ?? "");
const topicOwner = read(contract.topic_policy_owner ?? "");
const evaluator = read(contract.audience_evaluator ?? "");
const services = read(contract.services_file ?? "");
const crate = read(contract.crate_file ?? "");
const notificationSource = read(contract.notification_source_file ?? "");
const notificationContract = JSON.parse(read(contract.notification_visibility_contract ?? "") || "{}");
const notificationConsumer = JSON.parse(read(contract.notification_consumer_contract ?? "") || "{}");
const testSource = read(contract.test_file ?? "");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 3) {
  failures.push("forum topic audience visibility contract must use schema_version=3");
}
if (
  contract.task !== "FORUM-20J" ||
  contract.downstream_notification_task !== "FORUM-20K" ||
  contract.notification_consumer_task !== "FORUM-20O"
) {
  failures.push("forum topic audience visibility contract must connect FORUM-20J/K/O notification composition");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("source publication must not claim unexecuted richer visibility evidence");
}
for (const delivered of [
  "public_topic_created_notification",
  "recipient_target_open_notification",
  "recipient_mention_description_notification",
  "recipient_mention_audience_notification",
]) {
  if (contract.composition?.[delivered] !== true) {
    failures.push(`forum topic audience visibility contract must record ${delivered} as delivered`);
  }
}
for (const residual of [
  "topic and reply page filtering before count and pagination",
  "category and reply exact richer-audience composition",
  "create reply and moderate audience write policy",
  "host trust channel and group provider adapters",
  "visibility-scoped category and all-read mutations",
  "recipient-specific topic-created subscription filtering before pagination",
  "initially non-public topic-created descriptor materialization",
  "search index SEO and deep-link migration to the richer exact owner",
  "PostgreSQL concurrency and cross-consumer runtime evidence",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`forum topic audience visibility contract must keep ${residual} explicitly open`);
  }
}

const deliveredSlices = [
  "FORUM-20H",
  "FORUM-20I",
  "FORUM-20J",
  "FORUM-20K",
  "FORUM-20L",
  "FORUM-20M",
  "FORUM-20N",
  "FORUM-20O",
];
const planSync = contract.canonical_plan_sync ?? {};
if (planSync.required_ledger_through !== "FORUM-20O") {
  failures.push("forum topic audience visibility contract must require the canonical ledger through FORUM-20O");
}
if (JSON.stringify(planSync.required_delivered_sections) !== JSON.stringify(deliveredSlices)) {
  failures.push("forum topic audience visibility contract must require FORUM-20H through FORUM-20O delivered sections");
}
if (planSync.status === "pending") {
  if (planSync.current_plan_through !== "FORUM-20G") {
    failures.push("pending canonical plan synchronization must identify FORUM-20G as the current plan boundary");
  }
  requireText(
    plan,
    "FORUM-20A-G provide",
    "pending canonical plan synchronization must remain grounded in the current FORUM-20A-G ledger row",
  );
  for (const slice of deliveredSlices) {
    rejectText(
      plan,
      `### Delivered in \`${slice}\``,
      `canonical plan now contains ${slice}; update canonical_plan_sync before claiming pending through G`,
    );
  }
} else if (planSync.status === "synchronized") {
  requireText(plan, "FORUM-20A-O provide", "synchronized canonical plan must advance the FORUM-20 ledger through O");
  for (const slice of deliveredSlices) {
    requireText(
      plan,
      `### Delivered in \`${slice}\``,
      `synchronized canonical plan is missing the delivered ${slice} section`,
    );
  }
} else {
  failures.push("canonical_plan_sync.status must be pending or synchronized");
}

for (const marker of [
  "pub struct ForumTopicAudienceViewer",
  "pub fn public() -> Self",
  "pub fn authenticated(",
  "security.actor_kind != SecurityActorKind::User",
  "port_context.actor.kind != PortActorKind::User",
  "actor does not match the viewer",
  "pub struct ForumTopicAudienceVisibilityService",
  "pub fn without_facts_provider",
  "pub async fn is_topic_visible(",
  "ForumTopicVisibilityScope::storefront_for_viewer(",
  "ForumTopicVisibilityService::new(self.db.clone())",
  ".is_topic_visible(tenant_id, topic_id, &scope)",
  "find_topic(&self.db, tenant_id, topic_id)",
  "load_policy_for_topic(&self.db, tenant_id, &topic)",
  "for layer in &policy.inherited_category_layers",
  "policy.configured_constraints",
  ".resolve_for_constraints(",
  "ForumAudienceEvaluator::decide(",
  "facts.tenant_id = tenant_id",
  "facts.user_id = viewer.security.user_id",
]) {
  requireText(owner, marker, `exact richer topic visibility owner is missing ${marker}`);
}
for (const forbidden of [
  "crate::entities",
  "forum_category_audience_policy::",
  "forum_category_audience_role::",
  "forum_category_audience_channel::",
  "forum_category_audience_group::",
  "forum_category_audience_user::",
  "forum_topic_audience_policy::",
  "forum_topic_audience_role::",
  "forum_topic_audience_channel::",
  "forum_topic_audience_group::",
  "forum_topic_audience_user::",
]) {
  rejectText(owner, forbidden, `exact richer topic visibility owner must reuse policy owners instead of direct storage access ${forbidden}`);
}

const baseIndex = owner.indexOf(".is_topic_visible(tenant_id, topic_id, &scope)");
const policyIndex = owner.indexOf("load_policy_for_topic(&self.db, tenant_id, &topic)");
const providerIndex = owner.indexOf(".resolve_for_constraints(");
if (baseIndex < 0 || policyIndex < 0 || baseIndex > policyIndex) {
  failures.push("current exact storefront visibility must run before richer policy materialization");
}
if (baseIndex < 0 || providerIndex < 0 || baseIndex > providerIndex) {
  failures.push("current exact storefront visibility must run before optional owner facts calls");
}

for (const marker of [
  "pub struct ForumTopicVisibilityService",
  "pub async fn is_topic_visible",
  "self.hidden_category_ids_for_scope(tenant_id, scope)",
  "forum_topic::Column::Status.eq(TopicStatus::Open)",
  "all_topic_channel_access_subquery(tenant_id)",
]) {
  requireText(baseOwner, marker, `base topic visibility owner is missing ${marker}`);
}
for (const marker of ["pub(crate) async fn load_category_audience_policy", "effective_layers"]) {
  requireText(categoryOwner, marker, `category audience owner is missing ${marker}`);
}
for (const marker of ["async fn load_policy_for_topic", "inherited_category_layers", "configured_constraints"]) {
  requireText(topicOwner, marker, `topic audience owner is missing ${marker}`);
}
for (const marker of [
  "pub struct ForumAudienceFactsResolver",
  "pub struct ForumAudienceEvaluator",
  "ForumAudienceDecisionReason::ExplicitDeny",
]) {
  requireText(evaluator, marker, `audience evaluator contract is missing ${marker}`);
}
for (const marker of [
  "include!(\"topic_audience_visibility.rs\")",
  "ForumTopicAudienceViewer",
  "ForumTopicAudienceVisibilityService",
]) {
  requireText(services, marker, `forum services surface is missing ${marker}`);
}
for (const marker of ["ForumTopicAudienceViewer", "ForumTopicAudienceVisibilityService"]) {
  requireText(crate, marker, `forum crate surface is missing ${marker}`);
}

for (const marker of [
  "use crate::services::{ForumTopicAudienceViewer, ForumTopicAudienceVisibilityService};",
  "async fn load_topic_for_viewer(",
  "ForumTopicAudienceVisibilityService::new(self.db.clone(), self.facts_port.clone())",
  ".is_topic_visible(tenant_id, topic_id, None, viewer)",
  "async fn load_public_topic(",
  "ForumTopicAudienceViewer::public()",
  "async fn resolve_recipient_viewer(",
  "async fn load_mention_target_for_recipient(",
]) {
  requireText(notificationSource, marker, `notification source is missing richer topic visibility composition ${marker}`);
}
for (const forbidden of [
  "ForumTopicVisibilityScope::storefront(None)",
  "ForumTopicVisibilityService::new(",
  "forum_category_audience_policy",
  "forum_topic_audience_policy",
]) {
  rejectText(notificationSource, forbidden, `notification source must not bypass the richer owner with ${forbidden}`);
}

if (
  notificationContract.schema_version !== 6 ||
  notificationContract.task !== "FORUM-20K" ||
  notificationContract.downstream_task !== "FORUM-20O" ||
  notificationContract.composition?.exact_richer_public_owner !== true ||
  notificationContract.composition?.recipient_specific_target_open !== true ||
  notificationContract.composition?.recipient_specific_mention_description !== true ||
  notificationContract.composition?.recipient_specific_mention_audience !== true
) {
  failures.push("FORUM-20K notification visibility contract must record public and exact recipient composition through FORUM-20O");
}
if (
  notificationConsumer.schema_version !== 1 ||
  notificationConsumer.task !== "FORUM-20O" ||
  notificationConsumer.upstream_task !== "FORUM-20N" ||
  notificationConsumer.composition?.exact_mention_recipient_resolution !== true
) {
  failures.push("FORUM-20J topic visibility must remain synchronized with the FORUM-20O notification consumer");
}

for (const marker of [
  "exact_topic_visibility_conjoins_base_category_and_topic_audience_layers",
  "base visibility must fail before richer owner facts are requested",
  "public richer visibility should fail closed",
  "nonmatching local role should resolve as denied",
  "topic explicit deny should win",
  "missing exact membership should deny",
  "ForumError::CapabilityUnavailable",
  "cross-tenant topic should resolve as absent",
  "actor does not match",
  "assert_eq!(recorded.len(), 2)",
]) {
  requireText(testSource, marker, `richer topic visibility SQLite scenario is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum topic audience visibility verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum topic audience visibility contract is source-ready.");
