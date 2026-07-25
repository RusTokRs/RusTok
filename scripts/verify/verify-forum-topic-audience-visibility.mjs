#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT) : path.resolve(scriptDir, "../..");
const failures = [];
function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) { failures.push(`${relativePath}: required file is missing`); return ""; }
  return readFileSync(absolute, "utf8");
}
function requireText(source, marker, message) { if (!source.includes(marker)) failures.push(message); }
function rejectText(source, marker, message) { if (source.includes(marker)) failures.push(message); }

const contract = JSON.parse(read("crates/rustok-forum/contracts/forum-topic-audience-visibility.json") || "{}");
const owner = read(contract.owner_file ?? "");
const baseOwner = read(contract.base_visibility_owner ?? "");
const categoryOwner = read(contract.category_policy_owner ?? "");
const topicOwner = read(contract.topic_policy_owner ?? "");
const evaluator = read(contract.audience_evaluator ?? "");
const services = read(contract.services_file ?? "");
const crate = read(contract.crate_file ?? "");
const source = read(contract.notification_source_file ?? "");
const visibility = JSON.parse(read(contract.notification_visibility_contract ?? "") || "{}");
const consumer = JSON.parse(read(contract.notification_consumer_contract ?? "") || "{}");
const ownerTest = read(contract.test_file ?? "");
const consumerTest = read(contract.notification_consumer_test_file ?? "");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 4) failures.push("forum topic audience visibility contract must use schema_version=4");
if (contract.task !== "FORUM-20J" || contract.downstream_notification_task !== "FORUM-20K" || contract.notification_consumer_task !== "FORUM-20P") failures.push("topic visibility contract must connect FORUM-20J/K/P");
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") failures.push("topic visibility contract must not claim unexecuted evidence");
for (const field of [
  "exact_topic", "open_status", "route_channel", "public_authenticated_category_floor",
  "normalized_category_layers", "normalized_topic_layer", "role_and_explicit_user",
  "trust_channel_group_owner_facts", "public_topic_created_notification",
  "recipient_target_open_notification", "recipient_mention_description_notification",
  "recipient_mention_audience_notification", "recipient_topic_subscription_audience_notification",
]) if (contract.composition?.[field] !== true) failures.push(`topic visibility contract must record ${field}=true`);
for (const residual of [
  "topic and reply page filtering before count and pagination", "category and reply exact richer-audience composition",
  "create reply and moderate audience write policy", "host trust channel and group provider adapters",
  "visibility-scoped category and all-read mutations", "initially non-public topic-created descriptor materialization",
  "search index SEO and deep-link migration to the richer exact owner", "PostgreSQL concurrency and cross-consumer runtime evidence",
]) if (!contract.not_delivered?.includes(residual)) failures.push(`topic visibility contract must keep ${residual} open`);

const slices = ["FORUM-20H", "FORUM-20I", "FORUM-20J", "FORUM-20K", "FORUM-20L", "FORUM-20M", "FORUM-20N", "FORUM-20O", "FORUM-20P"];
const sync = contract.canonical_plan_sync ?? {};
if (sync.required_ledger_through !== "FORUM-20P" || JSON.stringify(sync.required_delivered_sections) !== JSON.stringify(slices)) failures.push("topic visibility contract must require FORUM-20H through FORUM-20P");
if (sync.status === "pending") {
  if (sync.current_plan_through !== "FORUM-20G") failures.push("pending plan boundary must remain FORUM-20G");
  requireText(plan, "FORUM-20A-G provide", "pending plan sync must remain grounded in FORUM-20A-G");
  for (const slice of slices) rejectText(plan, `### Delivered in \`${slice}\``, `canonical plan contains ${slice}; update plan sync metadata`);
} else if (sync.status !== "synchronized") failures.push("canonical_plan_sync.status must be pending or synchronized");

for (const marker of [
  "pub struct ForumTopicAudienceViewer", "pub fn public() -> Self", "pub fn authenticated(",
  "pub struct ForumTopicAudienceVisibilityService", "pub async fn is_topic_visible(",
  "ForumTopicVisibilityScope::storefront_for_viewer(", "ForumTopicVisibilityService::new(self.db.clone())",
  ".is_topic_visible(tenant_id, topic_id, &scope)", "load_policy_for_topic(&self.db, tenant_id, &topic)",
  "for layer in &policy.inherited_category_layers", "policy.configured_constraints",
  ".resolve_for_constraints(", "ForumAudienceEvaluator::decide(",
]) requireText(owner, marker, `exact topic visibility owner is missing ${marker}`);
for (const forbidden of ["crate::entities", "forum_category_audience_policy::", "forum_topic_audience_policy::"]) rejectText(owner, forbidden, `exact topic visibility owner must reuse policy owners instead of ${forbidden}`);
for (const marker of ["pub struct ForumTopicVisibilityService", "forum_topic::Column::Status.eq(TopicStatus::Open)", "all_topic_channel_access_subquery(tenant_id)"]) requireText(baseOwner, marker, `base visibility owner is missing ${marker}`);
for (const marker of ["pub(crate) async fn load_category_audience_policy", "effective_layers"]) requireText(categoryOwner, marker, `category owner is missing ${marker}`);
for (const marker of ["async fn load_policy_for_topic", "inherited_category_layers", "configured_constraints"]) requireText(topicOwner, marker, `topic owner is missing ${marker}`);
for (const marker of ["pub struct ForumAudienceFactsResolver", "pub struct ForumAudienceEvaluator", "ForumAudienceDecisionReason::ExplicitDeny"]) requireText(evaluator, marker, `audience evaluator is missing ${marker}`);
for (const marker of ["include!(\"topic_audience_visibility.rs\")", "ForumTopicAudienceViewer", "ForumTopicAudienceVisibilityService"]) requireText(services, marker, `services surface is missing ${marker}`);
for (const marker of ["ForumTopicAudienceViewer", "ForumTopicAudienceVisibilityService"]) requireText(crate, marker, `crate surface is missing ${marker}`);
for (const marker of [
  "async fn load_topic_for_viewer(", "ForumTopicAudienceVisibilityService::new(self.db.clone(), self.facts_port.clone())",
  ".is_topic_visible(tenant_id, topic_id, None, viewer)", "async fn load_public_topic(",
  "async fn resolve_recipient_viewer(", "async fn load_mention_target_for_recipient(",
  "async fn topic_subscription_recipient_visible(", "TOPIC_SUBSCRIPTION_AUDIENCE_ACTOR",
]) requireText(source, marker, `notification consumer is missing ${marker}`);
for (const forbidden of ["ForumTopicVisibilityScope::storefront(None)", "ForumTopicVisibilityService::new(", "forum_category_audience_policy", "forum_topic_audience_policy"]) rejectText(source, forbidden, `notification source must not bypass exact owner with ${forbidden}`);

if (visibility.schema_version !== 7 || visibility.task !== "FORUM-20K" || visibility.downstream_task !== "FORUM-20P" || visibility.composition?.recipient_specific_topic_subscription_audience !== true) failures.push("FORUM-20J must remain synchronized with FORUM-20K/P");
if (consumer.schema_version !== 1 || consumer.task !== "FORUM-20P" || consumer.upstream_task !== "FORUM-20O" || consumer.composition?.recipient_specific_topic_visibility !== true) failures.push("FORUM-20J must remain synchronized with FORUM-20P");
for (const marker of ["exact_topic_visibility_conjoins_base_category_and_topic_audience_layers", "topic explicit deny should win", "ForumError::CapabilityUnavailable", "actor does not match"]) requireText(ownerTest, marker, `exact visibility test is missing ${marker}`);
for (const marker of ["topic_subscription_audience_filters_exact_recipients_before_cursor_progress", "deny_user_ids: vec![denied_first, denied_fourth]", "BTreeSet::from([allowed_third, allowed_fifth])"]) requireText(consumerTest, marker, `topic subscription consumer test is missing ${marker}`);

if (failures.length > 0) {
  console.error("Forum topic audience visibility verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("Forum topic audience visibility contract is source-ready.");
