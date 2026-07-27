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
function between(source, start, end, label) {
  const from = source.indexOf(start);
  const to = source.indexOf(end, from + start.length);
  if (from < 0 || to < 0 || to <= from) {
    failures.push(`${label}: bounded section is missing`);
    return "";
  }
  return source.slice(from, to);
}

const contract = JSON.parse(
  read("crates/rustok-forum/contracts/forum-notification-topic-descriptor-materialization.json") ||
    "{}",
);
const source = read(contract.notification_source_file ?? "");
const test = read(contract.runtime_test_file ?? "");
const note = read(contract.owner_note ?? "");
const canonical = read(contract.canonical_plan ?? "");
const local = read(contract.notifications_local_plan ?? "");
const residual = JSON.parse(read(contract.residual_contract ?? "") || "{}");

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-20AP" ||
  contract.upstream_task !== "FORUM-20AO"
) {
  failures.push("topic descriptor materialization contract must identify FORUM-20AP after FORUM-20AO");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("topic descriptor materialization contract must not claim unexecuted evidence");
}
for (const key of [
  "topic_created_descriptor_materialization",
  "active_topic_required",
  "recipient_context_capability_gate",
  "public_only_fallback_without_capability",
  "initially_non_public_topic_supported",
  "minimal_identifier_template_data",
  "title_not_materialized",
  "body_not_materialized",
  "route_not_materialized",
  "exact_recipient_subscription_reauthorization",
  "current_category_and_topic_audience_recheck",
  "actor_exclusion_preserved",
  "bounded_raw_subscription_cursor_preserved",
  "missing_closed_deleted_topic_non_oracular",
  "owner_port_unchanged",
  "notification_storage_unchanged",
]) {
  if (contract.composition?.[key] !== true) failures.push(`contract must record ${key}`);
}
for (const key of ["migration_changed", "dependency_changed"]) {
  if (contract.composition?.[key] !== false) failures.push(`contract must keep ${key} false`);
}

for (const marker of [
  "async fn load_topic_for_description(",
  "if self.recipient_context_port.is_some()",
  "self.load_active_topic(tenant_id, topic_id).await?",
  "topic.status == TopicStatus::Open",
  "self.load_public_topic(tenant_id, topic_id).await",
  "load_topic_for_description(event.tenant_id, event.aggregate_id)",
  "load_topic_for_subscription_audience(event.tenant_id, event.aggregate_id)",
  "topic_subscription_recipient_visible(",
  ".limit((limit + 1) as u64)",
  "subscriptions.truncate(limit);",
  "forum_category_subscription::Column::UserId.ne(actor_id)",
]) {
  requireText(source, marker, `notification source is missing ${marker}`);
}

const describeBlock = between(
  source,
  "async fn describe_event(",
  "async fn resolve_audience(",
  "describe_event",
);
const topicDescribe = between(
  describeBlock,
  "TOPIC_CREATED_TYPE => {",
  "USER_MENTION_ADDED_TYPE => {",
  "topic-created describe branch",
);
for (const marker of [
  "load_topic_for_description(event.tenant_id, event.aggregate_id)",
  '("topic_id".to_string(), topic.id.to_string())',
  '("category_id".to_string(), topic.category_id.to_string())',
  "actor_id: event.actor_id.or(topic.author_id)",
]) {
  requireText(topicDescribe, marker, `topic-created descriptor is missing ${marker}`);
}
for (const forbidden of [
  '"title".to_string()',
  '"body".to_string()',
  '"route".to_string()',
  '"recipient_id".to_string()',
]) {
  rejectText(topicDescribe, forbidden, `topic-created descriptor must not materialize ${forbidden}`);
}

for (const marker of [
  "initially_non_public_topic_descriptor_requires_recipient_capability_and_reauthorizes",
  "without recipient capability an initially non-public topic must remain absent",
  "active initially non-public topic should materialize a descriptor",
  "descriptor.template_data.len(), 2",
  'for forbidden in ["title", "body", "route", "recipient_id", "audience"]',
  "page.recipients()[0].recipient_id, allowed_recipient",
  "closed initially non-public topic must not materialize a descriptor",
  "closed stale descriptor should be rechecked",
  "closed_page.recipients().is_empty()",
]) {
  requireText(test, marker, `SQLite descriptor scenario is missing ${marker}`);
}

for (const marker of [
  "# FORUM-20AP initially non-public topic descriptor materialization",
  "source-ready / unvalidated",
  "Descriptor creation is not recipient authorization",
  "historical public-only check",
  "not run by the implementation agent",
]) {
  requireText(note, marker, `owner note is missing ${marker}`);
}
for (const marker of [
  "FORUM-20A-AP provide",
  "### Delivered in `FORUM-20AP`",
  "Forum trust and Channel membership facts adapters",
]) {
  requireText(canonical, marker, `canonical plan is missing ${marker}`);
}
for (const marker of [
  "### `FORUM-20AP`",
  "initially non-public topic-created descriptors",
]) {
  requireText(local, marker, `Notifications local plan is missing ${marker}`);
}

if (
  residual.task !== "FORUM-20K" ||
  !residual.not_delivered?.includes(
    "initially non-public topic-created descriptor materialization",
  )
) {
  failures.push("FORUM-20AP must close the historical FORUM-20K descriptor residual");
}

if (failures.length > 0) {
  console.error("Forum notification topic descriptor materialization verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("Forum notification topic descriptor materialization contract is source-ready.");
