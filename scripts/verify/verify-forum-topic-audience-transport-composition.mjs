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
  const absolute = path.join(repoRoot, relativePath ?? "");
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

function requireOrder(source, first, second, message) {
  const firstIndex = source.indexOf(first);
  const secondIndex = source.indexOf(second);
  if (firstIndex < 0 || secondIndex < 0 || firstIndex >= secondIndex) {
    failures.push(message);
  }
}

const contractPath =
  "crates/rustok-forum/contracts/forum-topic-audience-transport-composition.json";
const contract = JSON.parse(read(contractPath) || "{}");
const context = read(contract.transport_context);
const graphqlQuery = read(contract.graphql_query);
const graphqlMarkRead = read(contract.graphql_mark_read);
const graphqlRuntime = read(contract.graphql_runtime);
const graphqlAdapter = read(contract.graphql_storefront_adapter);
const nativeAdapter = read(contract.native_storefront_adapter);
const readStateOwner = read(contract.read_state_owner);
const ownerNote = read(contract.owner_note);
const upstream = read(contract.upstream_contract);
const crateRoot = read("crates/rustok-forum/src/lib.rs");
const graphqlMod = read("crates/rustok-forum/src/graphql/mod.rs");

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-20BC" ||
  contract.upstream_task !== "FORUM-20BB" ||
  contract.downstream_task !== "FORUM-20BD"
) {
  failures.push("FORUM-20BC contract identity is invalid");
}

for (const marker of [
  "pub enum ForumTopicReadTransport",
  "pub enum ForumTopicReadOperation",
  "pub fn topic_read_audience_port_context(",
  "const FORUM_TOPIC_READ_FACTS_DEADLINE: Duration = Duration::from_secs(5);",
  "auth.tenant_id != tenant_id",
  "request.user_id != Some(auth.user_id)",
  ".with_deadline(FORUM_TOPIC_READ_FACTS_DEADLINE)",
  "context = context.with_claim(permission.to_string())",
  "context = context.with_channel(channel_slug.to_string())",
]) {
  requireText(context, marker, `topic read transport context is missing ${marker}`);
}

for (const marker of [
  "async fn forum_storefront_audience_topic(",
  "runtime.topic_audience_read_service",
  "ForumTopicReadTransport::Graphql",
  "ForumTopicReadOperation::SelectedTopic",
  "get_authenticated_storefront_visible_with_audience_context",
  "get_public_storefront_visible_with_locale_fallback",
  "Ok(topic.map(map_topic_response))",
]) {
  requireText(graphqlQuery, marker, `GraphQL exact topic query is missing ${marker}`);
}
requireOrder(
  graphqlQuery,
  "topic_read_audience_port_context(",
  "get_authenticated_storefront_visible_with_audience_context",
  "GraphQL must build the trusted context before calling the authenticated owner",
);

for (const marker of [
  "pub(crate) fn topic_audience_read_service(",
  "pub(crate) fn storefront_read_state_service(",
  "ForumTopicAudienceReadService::with_audience_facts",
  "ForumStorefrontReadStateService::with_audience_facts",
]) {
  requireText(graphqlRuntime, marker, `GraphQL runtime composition is missing ${marker}`);
}

for (const marker of [
  "mark_topic_read_current_audience_visible(",
  "ForumTopicAudienceReadService::with_audience_facts",
  "get_authenticated_storefront_visible_with_audience_context",
  "return Err(ForumError::TopicNotFound(topic_id));",
]) {
  requireText(readStateOwner, marker, `audience-safe mark-read owner is missing ${marker}`);
}
requireOrder(
  readStateOwner,
  "get_authenticated_storefront_visible_with_audience_context",
  "self.mark_topic_read_current(tenant_id, topic_id, security)",
  "mark-read must authorize the exact target before writing read state",
);

for (const marker of [
  "ForumTopicReadOperation::MarkRead",
  "topic_read_audience_port_context(",
  ".storefront_read_state_service(db.clone(), event_bus.clone())",
  ".mark_topic_read_current_audience_visible(",
]) {
  requireText(graphqlMarkRead, marker, `GraphQL mark-read composition is missing ${marker}`);
}
rejectText(
  graphqlMarkRead,
  ".mark_topic_read_current_visible(",
  "GraphQL mark-read still calls the compatibility visibility path",
);

for (const marker of [
  "forumStorefrontAudienceTopic(tenantId: $tenantId, id: $id, locale: $locale)",
  '#[serde(rename = "forumStorefrontAudienceTopic")]',
  "if selected_topic.is_some()",
  "markForumStorefrontTopicRead",
]) {
  requireText(graphqlAdapter, marker, `storefront GraphQL adapter is missing ${marker}`);
}
rejectText(
  graphqlAdapter,
  "{ forumStorefrontTopic(tenantId:",
  "module-owned GraphQL adapter still requests the legacy selected-topic field",
);

for (const marker of [
  "ForumTopicAudienceReadService::with_audience_facts",
  "load_audience_visible_topic(",
  "ForumTopicReadTransport::NativeServer",
  "ForumTopicReadOperation::SelectedTopic",
  "ForumTopicReadOperation::MarkRead",
  ".mark_topic_read_current_audience_visible(",
  "if selected_topic.is_some()",
]) {
  requireText(nativeAdapter, marker, `native storefront adapter is missing ${marker}`);
}
rejectText(
  nativeAdapter,
  ".mark_topic_read_current_visible(",
  "native mark-read still calls the compatibility visibility path",
);
rejectText(
  nativeAdapter,
  ".get_storefront_visible_with_locale_fallback(",
  "native selected-topic still calls the legacy visibility facade",
);

for (const marker of [
  "pub mod topic_read_transport;",
  "topic_read_audience_port_context",
  "ForumTopicReadOperation",
  "ForumTopicReadTransport",
]) {
  requireText(crateRoot, marker, `crate root is missing ${marker}`);
}
for (const marker of [
  "mod storefront_audience_topic;",
  "storefront_audience_topic::ForumStorefrontAudienceTopicQuery",
]) {
  requireText(graphqlMod, marker, `GraphQL module is missing ${marker}`);
}

for (const marker of [
  "FORUM-20BC",
  "ForumTopicAudienceReadService",
  "forumStorefrontAudienceTopic",
  "mark_topic_read_current_audience_visible",
  "five-second read deadline",
  "Replies are not requested or returned",
  "FORUM-20BD",
  "did not run tests",
]) {
  requireText(ownerNote, marker, `FORUM-20BC owner note is missing ${marker}`);
}

for (const marker of [
  '"downstream_task": "FORUM-20BC"',
  '"downstream_contract": "crates/rustok-forum/contracts/forum-topic-audience-transport-composition.json"',
]) {
  requireText(upstream, marker, `FORUM-20BB handoff is missing ${marker}`);
}

for (const [key, expected] of [
  ["native_selected_topic_uses_exact_owner", true],
  ["graphql_selected_topic_uses_exact_owner", true],
  ["graphql_storefront_adapter_uses_exact_field", true],
  ["native_mark_read_uses_exact_owner", true],
  ["graphql_mark_read_uses_exact_owner", true],
  ["authenticated_tenant_from_transport_context", true],
  ["authenticated_actor_from_transport_context", true],
  ["effective_locale_from_transport_context", true],
  ["route_channel_from_transport_context", true],
  ["claims_forwarded_to_owner_context", true],
  ["five_second_read_deadline", true],
  ["shared_optional_audience_facts_reused", true],
  ["public_selected_topic_skips_optional_facts", true],
  ["missing_and_denied_topic_non_oracular", true],
  ["replies_not_loaded_after_selected_topic_denial", true],
  ["topic_list_pagination_changed", false],
  ["unread_list_visibility_changed", false],
  ["reply_owner_read_changed", false],
  ["category_owner_read_changed", false],
  ["search_index_changed", false],
  ["seo_changed", false],
  ["deep_link_changed", false],
  ["migration_added", false],
  ["dependency_changed", false],
  ["public_dto_changed", false],
]) {
  if (contract.transport_boundary?.[key] !== expected) {
    failures.push(`FORUM-20BC transport_boundary.${key} must be ${expected}`);
  }
}

if (
  contract.documentation?.owner_note_updated !== true ||
  contract.documentation?.upstream_contract_updated !== true ||
  contract.documentation?.crate_api_updated !== false ||
  contract.documentation?.canonical_plan_updated !== false ||
  !contract.documentation?.synchronization_debt
) {
  failures.push("FORUM-20BC documentation handoff is incomplete");
}

if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("FORUM-20BC must not claim maintainer runtime execution");
}

if (failures.length > 0) {
  console.error("Forum topic audience transport verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum topic audience transports are source-ready.");
