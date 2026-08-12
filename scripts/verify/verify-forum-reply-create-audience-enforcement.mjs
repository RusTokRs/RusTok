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
  "crates/rustok-forum/contracts/forum-reply-create-audience-enforcement.json";
const contract = JSON.parse(read(contractPath) || "{}");
const upstream = JSON.parse(read(contract.policy_contract ?? "") || "{}");
const authorization = read(contract.authorization_service ?? "");
const facade = read(contract.reply_facade ?? "");
const rawOwner = read(contract.raw_reply_owner ?? "");
const rawInline = read(contract.raw_reply_owner_inline ?? "");
const services = read(contract.services_module ?? "");
const crateRoot = read(contract.crate_root ?? "");
const test = read(contract.runtime_test_file ?? "");
const note = read(contract.owner_note ?? "");
const topicLocalContractPath =
  "crates/rustok-forum/contracts/forum-topic-reply-create-audience-policy.json";
const topicLocalContract = existsSync(path.join(repoRoot, topicLocalContractPath))
  ? JSON.parse(read(topicLocalContractPath) || "{}")
  : null;

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-20AV" ||
  contract.upstream_task !== "FORUM-20AU"
) {
  failures.push("reply-create enforcement contract must connect FORUM-20AU/20AV");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("FORUM-20AV must not claim unexecuted verification evidence");
}
for (const key of [
  "reply_create_permission_first",
  "tenant_scoped_topic_category_resolution",
  "root_to_category_conjunction",
  "explicit_deny_precedence",
  "local_role_and_explicit_user_short_circuit",
  "exact_optional_owner_facts",
  "missing_context_fail_closed",
  "missing_provider_fail_closed",
  "legacy_create_gated",
  "inline_quote_create_gated",
  "authorization_before_raw_owner",
  "authorization_before_reply_body_relation_counter_stat_event_writes",
  "generic_public_denial",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`reply-create enforcement contract must record ${key}`);
  }
}
for (const key of [
  "transport_changed",
  "migration_changed",
  "public_dto_changed",
  "dependency_changed",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`reply-create enforcement contract must keep ${key}=false`);
  }
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20AU" ||
  upstream.composition?.reply_create_enforcement_changed !== false
) {
  failures.push("FORUM-20AV must remain grounded in the persistence-only FORUM-20AU contract");
}

for (const marker of [
  "pub struct ForumReplyCreateAudienceAuthorization",
  "pub struct ForumReplyCreateAudienceAuthorizationService",
  "enforce_scope(security, Resource::ForumReplies, Action::Create)?",
  "forum_topic::Entity::find_by_id(topic_id)",
  ".filter(forum_topic::Column::TenantId.eq(tenant_id))",
  "ForumAudienceEvaluator::decide(",
  "owner_facts_still_required(constraints, security)",
  "context.ok_or_else(||",
  "resolve_for_constraints(tenant_id, context, security, constraints)",
  "Forum reply creation is unavailable for the current audience",
]) {
  requireText(authorization, marker, `reply-create authorization is missing ${marker}`);
}
const historicalCategoryOnly =
  authorization.includes(
    "load_category_reply_create_audience_policy(&self.db, tenant_id, category_id)",
  ) && authorization.includes("for layer in policy.effective_layers");
const downstreamTopicNarrowing =
  authorization.includes(
    "load_topic_reply_create_audience_policy_for_topic(&self.db, tenant_id, &topic)",
  ) &&
  authorization.includes("for layer in policy.inherited_category_layers") &&
  authorization.includes("if let Some(constraints) = policy.configured_constraints") &&
  topicLocalContract?.schema_version === 1 &&
  topicLocalContract?.task === "FORUM-20AX" &&
  topicLocalContract?.composition?.root_category_to_topic_conjunction === true;
if (!historicalCategoryOnly && !downstreamTopicNarrowing) {
  failures.push(
    "reply-create authorization must retain FORUM-20AV category enforcement or the FORUM-20AX cumulative topic narrowing",
  );
}
for (const forbidden of [
  "forum_user_stats",
  "UserStatsService",
  "TopicService::adjust_reply_count_in_tx",
  "CategoryService::adjust_counters_in_tx",
  "publish_in_tx(",
  "forum_reply::ActiveModel",
  "forum_reply_body::ActiveModel",
]) {
  rejectText(
    authorization,
    forbidden,
    `reply-create authorization must not own writes or derive trust through ${forbidden}`,
  );
}

for (const marker of [
  "create_audience: ForumReplyCreateAudienceAuthorizationService",
  "pub fn with_audience_facts(",
  "pub async fn create_with_audience_context(",
  "pub async fn create_command_with_audience_context(",
  "async fn create_command_with_optional_audience_context(",
  ".require(tenant_id, topic_id, &security, context)",
  ".inner\n            .create_command(tenant_id, security, topic_id, input)",
]) {
  requireText(facade, marker, `reply facade is missing ${marker}`);
}
const authorizationIndex = facade.indexOf(
  ".require(tenant_id, topic_id, &security, context)",
);
const rawOwnerIndex = facade.indexOf(
  ".create_command(tenant_id, security, topic_id, input)",
);
if (authorizationIndex < 0 || rawOwnerIndex < 0 || authorizationIndex > rawOwnerIndex) {
  failures.push("reply-create audience authorization must run before the raw owner command");
}

for (const rawSource of [rawOwner, rawInline]) {
  rejectText(
    rawSource,
    "ForumReplyCreateAudienceAuthorizationService",
    "raw reply owners must not duplicate the public facade authorization boundary",
  );
  rejectText(
    rawSource,
    "SharedForumAudienceFactsPort",
    "raw reply owners must not consume optional audience facts directly",
  );
}

for (const marker of [
  "mod reply_create_audience_authorization;",
  "ForumReplyCreateAudienceAuthorization",
  "ForumReplyCreateAudienceAuthorizationService",
]) {
  requireText(services, marker, `services module is missing ${marker}`);
  if (marker !== "mod reply_create_audience_authorization;") {
    requireText(crateRoot, marker, `crate root is missing ${marker}`);
  }
}

for (const marker of [
  "reply_create_commands_enforce_inherited_audience_before_owner_writes",
  "category without reply-create audience should preserve compatibility",
  "matching role should not require owner facts or caller context",
  "explicit allow should short-circuit unresolved owner facts",
  "Forum reply creation is unavailable for the current audience",
  "denied or unresolved audience must not write reply or body rows",
  "create_command_with_audience_context(",
  "matching exact group facts should allow inline-command reply creation",
  "invalid exact caller context must fail before owner facts",
]) {
  requireText(test, marker, `reply-create SQLite proof is missing ${marker}`);
}

for (const marker of [
  "# FORUM-20AV reply-create audience enforcement",
  "source-ready / partially validated",
  "does not replace or duplicate its backlog",
  "GraphQL and REST now compose exact authenticated transport",
  "source verifier was executed on 2026-08-11",
  "Canonical plan synchronization",
]) {
  requireText(note, marker, `reply-create owner note is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum reply-create audience enforcement verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("Forum reply-create audience enforcement contract is source-ready.");
