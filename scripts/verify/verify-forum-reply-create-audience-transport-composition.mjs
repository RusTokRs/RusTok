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
  if (!relativePath || !existsSync(absolute)) {
    failures.push(`${relativePath || "<missing path>"}: required file is missing`);
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
  "crates/rustok-forum/contracts/forum-reply-create-audience-transport-composition.json";
const contract = JSON.parse(read(contractPath) || "{}");
const transport = read(contract.transport_context_file ?? "");
const graphqlRuntime = read(contract.graphql_runtime_file ?? "");
const graphqlStandard = read(contract.graphql_standard_mutation_file ?? "");
const graphqlCommand = read(contract.graphql_command_mutation_file ?? "");
const graphqlTypes = read("crates/rustok-forum/src/graphql/types.rs");
const httpRuntime = read(contract.http_runtime_file ?? "");
const httpStandard = read(contract.http_standard_create_file ?? "");
const httpCommand = read(contract.http_command_create_file ?? "");
const owner = read(contract.owner_enforcement_file ?? "");
const note = read(contract.owner_note ?? "");
const crateApi = read(contract.crate_api ?? "");

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-20AW" ||
  contract.upstream_task !== "FORUM-20AV"
) {
  failures.push("reply-create transport contract must identify FORUM-20AW after FORUM-20AV");
}
if (contract.verification?.execution_status !== "source_verified_runtime_pending") {
  failures.push("reply-create transport contract must record current source verification status");
}
for (const key of [
  "shared_exact_transport_context_builder",
  "auth_tenant_request_identity_validation",
  "dto_identity_forbidden",
  "read_deadline_semantics",
  "permission_claim_forwarding",
  "resolved_route_channel_forwarding",
  "graphql_standard_create_composed",
  "graphql_command_create_composed",
  "rest_standard_create_composed",
  "rest_command_create_composed",
  "graphql_runtime_facts_consumed",
  "http_runtime_facts_consumed",
  "host_extension_facts_reused",
  "missing_provider_fail_closed",
  "local_policy_semantics_preserved",
  "owner_authorization_before_writes_preserved",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`reply-create transport contract must record ${key}`);
  }
}
for (const key of [
  "migration_changed",
  "reply_create_dto_changed",
  "forum_groups_dependency_added",
  "trust_facts_adapter_added",
  "topic_local_narrowing_added",
  "moderation_audience_added",
  "canonical_plan_rewritten",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`reply-create transport contract must keep ${key} false`);
  }
}

for (const marker of [
  "enum ForumReplyCreateTransport",
  "Graphql",
  "Rest",
  "reply_create_audience_port_context",
  "auth.tenant_id != tenant_id",
  "request.tenant_id != tenant_id",
  "request.user_id != Some(auth.user_id)",
  "auth.port_actor()",
  "with_deadline(FORUM_REPLY_CREATE_FACTS_DEADLINE)",
  "context.with_claim(permission.to_string())",
  "request.channel_slug.as_deref()",
  "context.with_channel(channel_slug.to_string())",
  "forum-{}-reply-create-{}-{}",
]) {
  requireText(transport, marker, `shared reply-create transport context is missing ${marker}`);
}
for (const marker of [
  "exact_rest_context_uses_authenticated_identity_deadline_claims_and_channel",
  "graphql_context_without_http_request_uses_authenticated_tenant_and_fallback_locale",
  "mismatched_auth_request_tenant_or_user_fails_before_owner_facts",
  "context.deadline_ms, Some(5_000)",
  "context.channel.as_deref(), Some(\"members\")",
]) {
  requireText(transport, marker, `reply-create transport inline contract test is missing ${marker}`);
}

for (const marker of [
  "pub struct ForumGraphqlRuntimeData",
  "audience_facts: Option<SharedForumAudienceFactsPort>",
  "inputs.shared_get::<SharedForumAudienceFactsPort>()",
  "fn reply_service(",
  "ReplyService::with_audience_facts",
  "ReplyService::new",
  "schema_factory_consumes_host_published_audience_facts_without_db_discovery",
  "schema_factory_preserves_optional_provider_absence",
]) {
  requireText(graphqlRuntime, marker, `Forum GraphQL runtime data is missing ${marker}`);
}

const standardGraphqlCreate = between(
  graphqlStandard,
  "async fn create_forum_reply(",
  "async fn set_forum_topic_vote(",
  "standard GraphQL reply-create",
);
const commandGraphqlCreate = between(
  graphqlCommand,
  "async fn create_forum_reply_with_quotes(",
  "async fn update_forum_reply_with_quotes(",
  "command GraphQL reply-create",
);
for (const [source, label, commandMarker] of [
  [standardGraphqlCreate, "standard GraphQL reply-create", ".create_with_audience_context("],
  [commandGraphqlCreate, "command GraphQL reply-create", ".create_command_with_audience_context("],
]) {
  for (const marker of [
    "ForumGraphqlRuntimeData",
    "reply_create_audience_port_context(",
    "ForumReplyCreateTransport::Graphql",
    "ctx.data_opt::<rustok_api::RequestContext>()",
    ".reply_service(db.clone(), event_bus.clone())",
    commandMarker,
  ]) {
    requireText(source, marker, `${label} is missing ${marker}`);
  }
}
for (const marker of [
  "tenant_id: Option<Uuid>",
  "resolve_tenant_scope(tenant, tenant_id)?",
]) {
  requireText(
    standardGraphqlCreate,
    marker,
    `standard GraphQL reply-create must derive optional tenant scope: ${marker}`,
  );
}

for (const marker of [
  "audience_facts: Option<SharedForumAudienceFactsPort>",
  "runtime.shared_get::<SharedForumAudienceFactsPort>()",
  "fn reply_service(&self) -> crate::ReplyService",
  "ReplyService::with_audience_facts",
  "ReplyService::new",
]) {
  requireText(httpRuntime, marker, `Forum HTTP runtime is missing ${marker}`);
}
const standardRestCreate = between(
  httpStandard,
  "pub async fn create_reply(",
  "pub async fn update_reply(",
  "standard REST reply-create",
);
const commandRestCreate = between(
  httpCommand,
  "pub async fn create_reply(",
  "pub async fn update_reply(",
  "command REST reply-create",
);
for (const [source, label, commandMarker] of [
  [standardRestCreate, "standard REST reply-create", ".create_with_audience_context("],
  [commandRestCreate, "command REST reply-create", ".create_command_with_audience_context("],
]) {
  for (const marker of [
    "request_context: RequestContext",
    ".reply_service()",
    "reply_create_audience_port_context(",
    "ForumReplyCreateTransport::Rest",
    commandMarker,
  ]) {
    requireText(source, marker, `${label} is missing ${marker}`);
  }
}

const ownerCreateHelper = between(
  owner,
  "async fn create_command_with_optional_audience_context(",
  "pub async fn get(",
  "reply-create owner helper",
);
for (const marker of [
  ".require(tenant_id, topic_id, &security, context)",
  ".create_command(tenant_id, security, topic_id, input)",
]) {
  requireText(ownerCreateHelper, marker, `reply-create owner helper is missing ${marker}`);
}
if (
  ownerCreateHelper.indexOf(".require(tenant_id, topic_id, &security, context)") >
  ownerCreateHelper.indexOf(".create_command(tenant_id, security, topic_id, input)")
) {
  failures.push("transport composition must preserve reply audience authorization before owner writes");
}

const standardInput = between(
  graphqlTypes,
  "pub struct CreateForumReplyInput",
  "pub struct CreateForumCategoryInput",
  "standard GraphQL reply-create input",
);
const commandInput = between(
  graphqlCommand,
  "pub struct CreateForumReplyWithQuotesInput",
  "pub struct UpdateForumReplyWithQuotesInput",
  "command GraphQL reply-create input",
);
for (const [input, label] of [
  [standardInput, "standard GraphQL reply-create input"],
  [commandInput, "command GraphQL reply-create input"],
]) {
  for (const forbidden of ["user_id", "actor_id", "recipient_id", "request_tenant_id"]) {
    rejectText(input, forbidden, `${label} must not accept ${forbidden}`);
  }
}

for (const marker of [
  "# FORUM-20AW reply-create audience transport composition",
  "source-verified / runtime evidence pending",
  "Tenant and actor identity come only from authenticated transport extensions",
  "same optional `SharedForumAudienceFactsPort`",
  "No Forum-to-Groups crate dependency was added",
  "Canonical plan synchronization",
  "Source checks executed on 2026-08-11",
]) {
  requireText(note, marker, `reply-create transport owner note is missing ${marker}`);
}
for (const marker of [
  "ReplyService::create_with_audience_context",
  "ReplyService::create_command_with_audience_context",
  "ReplyService::with_audience_facts",
  "FORUM-20AW",
  "GraphQL and REST reply-create",
]) {
  requireText(crateApi, marker, `Forum CRATE_API is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum reply-create audience transport composition verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("Forum reply-create audience transport composition contract is source-ready.");
