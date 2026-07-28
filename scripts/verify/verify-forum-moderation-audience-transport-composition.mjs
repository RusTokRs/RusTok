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

function section(source, start, end) {
  const startIndex = source.indexOf(start);
  if (startIndex < 0) {
    failures.push(`missing section start: ${start}`);
    return "";
  }
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (endIndex < 0) {
    failures.push(`missing section end after ${start}: ${end}`);
    return source.slice(startIndex);
  }
  return source.slice(startIndex, endIndex);
}

const contractPath =
  "crates/rustok-forum/contracts/forum-moderation-audience-transport-composition.json";
const contract = JSON.parse(read(contractPath) || "{}");
const helper = read(contract.transport_helper ?? "");
const graphql = read(contract.graphql_handler ?? "");
const graphqlRuntime = read(contract.graphql_runtime ?? "");
const rest = read(contract.rest_handler ?? "");
const restRuntime = read(contract.rest_runtime ?? "");
const authorization = read(contract.owner_authorization ?? "");
const owner = read(contract.owner_service ?? "");
const crateRoot = read(contract.crate_root ?? "");
const crateApi = read(contract.crate_api ?? "");
const note = read(contract.owner_note ?? "");

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-20AZ" ||
  contract.upstream_task !== "FORUM-20AY"
) {
  failures.push("moderation transport contract must identify FORUM-20AZ after FORUM-20AY");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("FORUM-20AZ must not claim unexecuted verification evidence");
}

for (const key of [
  "existing_graphql_mark_solution_composed",
  "existing_graphql_clear_solution_composed",
  "existing_rest_mark_solution_composed",
  "existing_rest_clear_solution_composed",
  "graphql_authenticated_admission",
  "rest_authenticated_admission",
  "exact_topic_author_decided_by_owner",
  "non_author_moderator_scope_decided_by_owner",
  "transport_precheck_does_not_block_topic_author",
  "tenant_argument_validated_against_tenant_context",
  "request_tenant_validated_against_authenticated_tenant",
  "request_actor_validated_against_authenticated_user",
  "owner_context_tenant_validated_before_target_lookup",
  "owner_context_actor_validated_before_target_lookup",
  "authenticated_permission_claims_forwarded",
  "request_locale_or_tenant_fallback_forwarded",
  "resolved_route_channel_forwarded",
  "five_second_facts_deadline",
  "bounded_unique_correlation_id",
  "graphql_runtime_reuses_host_audience_facts",
  "rest_runtime_reuses_host_audience_facts",
  "missing_provider_remains_fail_closed_for_external_facts",
  "locally_decidable_policy_remains_compatible",
  "authorization_before_solution_write",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`moderation transport contract must record ${key}`);
  }
}

for (const key of [
  "new_moderation_endpoints_added",
  "public_dto_changed",
  "openapi_shape_changed",
  "route_shape_changed",
  "migration_added",
  "dependency_changed",
  "host_server_source_changed",
  "trust_owner_state_added",
  "trust_derived_from_forum_user_stats",
  "approve_reject_hide_transport_added",
  "pin_lock_status_transport_added",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`moderation transport contract must keep ${key}=false`);
  }
}

for (const marker of [
  "pub(crate) enum ForumModerationTransport",
  "Graphql",
  "Rest",
  "pub(crate) fn moderation_audience_port_context(",
  "auth.tenant_id != tenant_id",
  "request.tenant_id != tenant_id",
  "request.user_id != Some(auth.user_id)",
  "request.locale.trim()",
  "fallback_locale.trim()",
  "auth.port_actor()",
  "auth.session_id",
  "Uuid::new_v4()",
  ".with_deadline(FORUM_MODERATION_FACTS_DEADLINE)",
  "context.with_claim(permission.to_string())",
  "request.channel_slug.as_deref()",
  "context.with_channel(channel_slug.to_string())",
  "Duration::from_secs(5)",
]) {
  requireText(helper, marker, `moderation transport helper is missing ${marker}`);
}
for (const forbidden of [
  "forum_user_stats",
  "TopicService",
  "ReplyService",
  "ModerationService",
  "SharedForumAudienceFactsPort",
]) {
  rejectText(helper, forbidden, `transport helper must not own policy, facts, or writes through ${forbidden}`);
}

const graphqlMark = section(
  graphql,
  "async fn mark_forum_topic_solution(",
  "async fn clear_forum_topic_solution(",
);
const graphqlClear = section(
  graphql,
  "async fn clear_forum_topic_solution(",
  "async fn create_forum_category(",
);
for (const [name, source, ownerCall] of [
  ["GraphQL mark solution", graphqlMark, "mark_solution_with_audience_context("],
  ["GraphQL clear solution", graphqlClear, "clear_solution_with_audience_context("],
]) {
  for (const marker of [
    "require_module_enabled(ctx, MODULE_SLUG).await?",
    ".data::<AuthContext>()",
    "resolve_tenant_scope(tenant, Some(tenant_id))?",
    "moderation_audience_port_context(",
    "ForumModerationTransport::Graphql",
    "ctx.data_opt::<rustok_api::RequestContext>()",
    ".data_opt::<ForumGraphqlRuntimeData>()",
    ".moderation_service(db.clone(), event_bus.clone())",
    ownerCall,
  ]) {
    requireText(source, marker, `${name} is missing ${marker}`);
  }
  rejectText(
    source,
    "require_forum_permission(",
    `${name} must not block the exact topic author before owner authorization`,
  );
  rejectText(source, ".mark_solution(", `${name} must not call context-free mark_solution`);
  rejectText(source, ".clear_solution(", `${name} must not call context-free clear_solution`);
}

for (const marker of [
  "pub async fn mark_topic_solution(",
  "pub async fn clear_topic_solution(",
  "auth: AuthContext",
  "request_context: RequestContext",
  "moderation_audience_port_context(",
  "ForumModerationTransport::Rest",
  ".moderation_service()",
  ".mark_solution_with_audience_context(",
  ".clear_solution_with_audience_context(",
]) {
  requireText(rest, marker, `REST moderation composition is missing ${marker}`);
}
for (const forbidden of [
  "ensure_solution_permission",
  "has_any_effective_permission",
  "Permission::FORUM_TOPICS_UPDATE",
  "Permission::FORUM_TOPICS_MODERATE",
  ".mark_solution(",
  ".clear_solution(",
]) {
  rejectText(rest, forbidden, `REST transport must defer exact author/moderator admission to owner: ${forbidden}`);
}

for (const [runtimeName, source] of [
  ["GraphQL", graphqlRuntime],
  ["REST", restRuntime],
]) {
  for (const marker of [
    "audience_facts: Option<SharedForumAudienceFactsPort>",
    "fn moderation_service(",
    "ModerationService::with_audience_facts",
    "ModerationService::new",
  ]) {
    requireText(source, marker, `${runtimeName} runtime is missing ${marker}`);
  }
}
for (const marker of [
  "pub mod moderation;",
  "axum::routing::post(moderation::mark_topic_solution)",
  "axum::routing::delete(moderation::clear_topic_solution)",
]) {
  requireText(restRuntime, marker, `REST router is missing ${marker}`);
}
for (const oldRoute of [
  "axum::routing::post(topics::mark_topic_solution)",
  "axum::routing::delete(topics::clear_topic_solution)",
]) {
  rejectText(restRuntime, oldRoute, `REST router still uses legacy solution handler: ${oldRoute}`);
}

for (const marker of [
  "fn exact_transport_context(",
  "context.tenant_id != tenant_id.to_string()",
  "context.actor.kind != PortActorKind::User",
  "context.actor.id != user_id.to_string()",
]) {
  requireText(authorization, marker, `owner authorization is missing ${marker}`);
}
const topicContextIndex = authorization.indexOf(
  "let context = exact_transport_context(tenant_id, security, context)?;",
);
const topicLookupIndex = authorization.indexOf("let topic = forum_topic::Entity::find_by_id(topic_id)");
if (topicContextIndex < 0 || topicLookupIndex < 0 || topicContextIndex > topicLookupIndex) {
  failures.push("topic moderation context identity must be validated before target lookup");
}
const replyContextIndex = authorization.indexOf(
  "let context = exact_transport_context(tenant_id, security, context)?;",
  topicContextIndex + 1,
);
const replyLookupIndex = authorization.indexOf("let reply = forum_reply::Entity::find_by_id(reply_id)");
if (replyContextIndex < 0 || replyLookupIndex < 0 || replyContextIndex > replyLookupIndex) {
  failures.push("reply moderation context identity must be validated before target lookup");
}
rejectText(
  authorization,
  "forum_user_stats",
  "moderation authorization must not derive trust from forum_user_stats",
);

for (const marker of [
  "pub async fn mark_solution_with_audience_context(",
  "pub async fn clear_solution_with_audience_context(",
  "if !is_exact_topic_author(&security, topic.author_id)",
  ".require_topic(tenant_id, topic_id, &security, context)",
  "let txn = self.db.begin().await?",
]) {
  requireText(owner, marker, `ModerationService owner is missing ${marker}`);
}
const ownerAuthorizationIndex = owner.indexOf(
  ".require_topic(tenant_id, topic_id, &security, context)",
);
const ownerWriteIndex = owner.indexOf("forum_solution::Entity::delete_many()");
if (
  ownerAuthorizationIndex < 0 ||
  ownerWriteIndex < 0 ||
  ownerAuthorizationIndex > ownerWriteIndex
) {
  failures.push("moderator audience authorization must remain before solution writes");
}

requireText(crateRoot, "mod moderation_transport;", "Forum crate root must register moderation transport helper");
for (const forbidden of [
  "pub mod moderation_transport;",
  "pub use moderation_transport",
]) {
  rejectText(crateRoot, forbidden, "moderation transport helper must remain crate-private");
}

for (const marker of [
  "# FORUM-20AZ moderation audience transport composition",
  "source-ready / unvalidated",
  "existing GraphQL `markForumTopicSolution`",
  "existing REST `POST /api/forum/topics/{topic_id}/solution/{reply_id}`",
  "exact tenant-scoped topic author",
  "No new GraphQL field, REST route, OpenAPI shape",
  "Trust remains blocked on `FORUM-26`",
  "canonical `crates/rustok-forum/docs/implementation-plan.md` is intentionally not rewritten",
  "were not run by the implementation agent",
]) {
  requireText(note, marker, `FORUM-20AZ owner note is missing ${marker}`);
}
for (const marker of [
  "FORUM-20AZ",
  "moderation_audience_port_context",
  "mark_solution_with_audience_context",
  "clear_solution_with_audience_context",
  "exact tenant-scoped topic author",
]) {
  requireText(crateApi, marker, `Forum CRATE_API is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum moderation audience transport verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum moderation audience transport contract is source-ready.");
