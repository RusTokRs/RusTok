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

const contractPath =
  "crates/rustok-forum/contracts/forum-topic-create-audience-enforcement.json";
const contract = JSON.parse(read(contractPath) || "{}");
const authorization = read(contract.authorization_service_file ?? "");
const policy = read(contract.policy_service_file ?? "");
const facade = read(contract.topic_facade_file ?? "");
const services = read(contract.services_module ?? "");
const crateRoot = read(contract.crate_root ?? "");
const test = read(contract.runtime_test_file ?? "");
const note = read(contract.owner_note ?? "");
const crateApi = read(contract.crate_api ?? "");
const plan = read(contract.canonical_plan ?? "");

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-20AR" ||
  contract.upstream_task !== "FORUM-20AQ"
) {
  failures.push("topic-create enforcement contract must identify FORUM-20AR after FORUM-20AQ");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("topic-create enforcement contract must not claim unexecuted evidence");
}
for (const key of [
  "create_permission_before_policy_read",
  "categories_without_policy_unchanged",
  "root_to_category_conjunction",
  "local_role_allow_deny_without_context",
  "explicit_deny_wins",
  "owner_facts_only_when_locally_unresolved",
  "exact_port_context_required_for_owner_facts",
  "missing_context_fails_closed",
  "missing_facts_provider_fails_closed",
  "generic_forbidden_without_selector_disclosure",
  "authorization_before_topic_writes",
  "authorization_before_counters_and_events",
  "legacy_create_methods_enforced",
  "context_aware_create_methods_published",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`topic-create enforcement contract must record ${key}`);
  }
}
for (const key of ["migration_changed", "transport_changed", "dependency_changed"]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`topic-create enforcement contract must keep ${key} false`);
  }
}

for (const marker of [
  "pub struct ForumTopicCreateAudienceAuthorization",
  "pub struct ForumTopicCreateAudienceAuthorizationService",
  "enforce_scope(security, Resource::ForumTopics, Action::Create)?",
  "load_category_topic_create_audience_policy",
  "for layer in policy.effective_layers",
  "owner_facts_still_required",
  "context.ok_or_else",
  "FORUM_AUDIENCE_FACTS_CAPABILITY_UNAVAILABLE",
  "ForumAudienceEvaluator::decide",
  "Forum topic creation is unavailable for the current audience",
  "denied_by_category_id: Some(layer.category_id)",
]) {
  requireText(authorization, marker, `topic-create authorization owner is missing ${marker}`);
}
requireText(
  authorization,
  "constraints.deny_user_ids.binary_search(&user_id).is_ok()",
  "topic-create authorization must let explicit deny short-circuit owner facts",
);
requireText(
  authorization,
  "constraints.allow_user_ids.binary_search(&user_id).is_ok()",
  "topic-create authorization must let explicit allow short-circuit owner facts",
);
requireText(
  authorization,
  "constraints.roles_any.contains(&security.role)",
  "topic-create authorization must let a matching role short-circuit owner facts",
);
rejectText(
  authorization,
  "forum_topic::ActiveModel",
  "topic-create authorization service must not write topic rows",
);

requireText(
  policy,
  "pub(crate) async fn load_category_topic_create_audience_policy",
  "topic-create enforcement must reuse the normalized policy owner loader",
);
for (const marker of [
  "ForumTopicCreateAudienceAuthorizationService",
  "SharedForumAudienceFactsPort",
  "pub fn with_audience_facts",
  "pub async fn create_with_audience_context",
  "pub async fn create_command_with_audience_context",
]) {
  requireText(facade, marker, `TopicService facade is missing ${marker}`);
}
const createBlock = between(
  facade,
  "async fn create_command_with_optional_audience_context(",
  "pub async fn get(",
  "topic-create facade helper",
);
for (const marker of [
  ".require(tenant_id, input.category_id, &security, context)",
  ".create_command(tenant_id, security, input)",
]) {
  requireText(createBlock, marker, `TopicService create helper is missing ${marker}`);
}
const requireIndex = createBlock.indexOf(
  ".require(tenant_id, input.category_id, &security, context)",
);
const createIndex = createBlock.indexOf(
  ".create_command(tenant_id, security, input)",
);
if (requireIndex < 0 || createIndex < 0 || requireIndex > createIndex) {
  failures.push("TopicService must authorize before delegating to the topic write owner");
}

for (const marker of [
  "mod topic_create_audience_authorization;",
  "ForumTopicCreateAudienceAuthorization",
  "ForumTopicCreateAudienceAuthorizationService",
]) {
  requireText(services, marker, `Forum services module is missing ${marker}`);
}
for (const marker of [
  "ForumTopicCreateAudienceAuthorization",
  "ForumTopicCreateAudienceAuthorizationService",
]) {
  requireText(crateRoot, marker, `Forum crate root is missing ${marker}`);
}

for (const marker of [
  "topic_create_command_enforces_inherited_audience_before_writes",
  "category without topic-create audience should preserve compatibility",
  "matching local role should not require owner facts or caller context",
  "explicit allow should short-circuit unresolved owner facts",
  "explicit deny topic-create layer should persist",
  "count_before_explicit_deny",
  "explicit-denied",
  "count_before_denials",
  "Err(ForumError::CapabilityUnavailable",
  "missing-context",
  "matching exact group facts should allow topic creation",
  "invalid exact caller context must fail before owner facts",
  "actor does not match",
  "tenant does not match",
]) {
  requireText(test, marker, `topic-create enforcement SQLite proof is missing ${marker}`);
}

for (const marker of [
  "# FORUM-20AR topic-create audience enforcement",
  "source-ready / unvalidated",
  "before topic, translation, relation, counter, user-stat, or domain-event writes",
  "does **not** compose GraphQL or REST caller contexts",
  "not run by the implementation agent",
]) {
  requireText(note, marker, `topic-create enforcement owner note is missing ${marker}`);
}
for (const marker of [
  "ForumTopicCreateAudienceAuthorizationService",
  "create_with_audience_context",
  "with_audience_facts",
]) {
  requireText(crateApi, marker, `Forum CRATE_API is missing ${marker}`);
}
for (const marker of [
  "FORUM-20A-AR provide",
  "### Delivered in `FORUM-20AR`",
  "GraphQL/REST/runtime topic-create audience composition",
]) {
  requireText(plan, marker, `canonical Forum plan is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum topic-create audience enforcement verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("Forum topic-create audience enforcement contract is source-ready.");
