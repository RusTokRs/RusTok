#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];
const paths = {
  contract: "crates/rustok-forum/contracts/forum-search-current-channel-filter.json",
  note: "crates/rustok-forum/docs/forum-23b2f4-search-current-channel-filter.md",
  projection: "crates/rustok-forum/src/search_projection.rs",
  topicOwner: "crates/rustok-forum/src/services/topic_owner.rs",
  topicInline: "crates/rustok-forum/src/services/topic_inline.rs",
  filter: "crates/rustok-search/src/forum_current_channel_filter.rs",
  execution: "crates/rustok-search/src/forum_storefront_execution.rs",
  forumInbox: "crates/rustok-search/src/forum_inbox.rs",
  forumProjector: "crates/rustok-search/src/forum_projector.rs",
  searchLib: "crates/rustok-search/src/lib.rs",
  graphqlOwner: "crates/rustok-search/src/graphql/forum_storefront.rs",
  graphqlTypes: "crates/rustok-search/src/graphql/types.rs",
  storefrontModel: "crates/rustok-search/storefront/src/model.rs",
  graphqlAdapter:
    "crates/rustok-search/storefront/src/transport/forum_graphql_adapter.rs",
  nativeAdapter:
    "crates/rustok-search/storefront/src/transport/forum_native_server_adapter.rs",
  transportFacade: "crates/rustok-search/storefront/src/transport/mod.rs",
  engine: "crates/rustok-search/src/engine.rs",
};

function read(relativePath) {
  const target = path.join(root, relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(target, "utf8");
}

function requireAll(source, markers, label) {
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
  }
}

function rejectAll(source, markers, label) {
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
  }
}

function parseJson(relativePath) {
  try {
    return JSON.parse(read(relativePath));
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return null;
  }
}

function functionBody(source, functionName) {
  const signature = new RegExp(
    `(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${functionName}(?:<[^>]*>)?\\s*\\(`,
  );
  const match = signature.exec(source);
  if (!match) {
    failures.push(`missing function ${functionName}`);
    return "";
  }
  const openBrace = source.indexOf("{", match.index);
  if (openBrace < 0) {
    failures.push(`missing body for ${functionName}`);
    return "";
  }
  let depth = 0;
  for (let index = openBrace; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated body for ${functionName}`);
  return "";
}

const contract = parseJson(paths.contract);
const note = read(paths.note);
const projection = read(paths.projection);
const topicOwner = read(paths.topicOwner);
const topicInline = read(paths.topicInline);
const filter = read(paths.filter);
const execution = read(paths.execution);
const forumInbox = read(paths.forumInbox);
const forumProjector = read(paths.forumProjector);
const searchLib = read(paths.searchLib);
const graphqlOwner = read(paths.graphqlOwner);
const graphqlTypes = read(paths.graphqlTypes);
const storefrontModel = read(paths.storefrontModel);
const graphqlAdapter = read(paths.graphqlAdapter);
const nativeAdapter = read(paths.nativeAdapter);
const transportFacade = read(paths.transportFacade);
const engine = read(paths.engine);

requireAll(projection, [
  '"channel_slugs": topic.channel_slugs',
  "let topic_channel_slugs = topic.channel_slugs.clone();",
  '"topic_channel_slugs": topic_channel_slugs',
], paths.projection);

requireAll(filter, [
  "pub(crate) struct ForumStorefrontCurrentChannelFilter",
  "pub channel_slug: Option<String>",
  '"forum_topic" => "channel_slugs"',
  '"forum_reply" => "topic_channel_slugs"',
  "projected_channels.contains(&expected_channel)",
  "exact_current_channel_matches_topics_and_parent_scoped_replies",
  "missing_or_malformed_projection_fails_closed",
], paths.filter);
rejectAll(filter, ["rustok_forum", "forum_topic::", "forum_reply::", "group_ids"], paths.filter);

const ownerUpdate = functionBody(topicOwner, "update");
requireAll(ownerUpdate, [
  "update_with_inline_relations",
  "input.into()",
], `${paths.topicOwner}: public topic update`);
rejectAll(ownerUpdate, ["update_with_relations"], `${paths.topicOwner}: legacy update path`);

const inlineUpdate = functionBody(topicInline, "update_with_inline_relations");
requireAll(inlineUpdate, [
  "publish_forum_topic_projection_in_tx",
  "txn.commit().await?;",
], `${paths.topicInline}: transactional projection invalidation`);
if (
  inlineUpdate.indexOf("publish_forum_topic_projection_in_tx") >
  inlineUpdate.indexOf("txn.commit().await?;")
) {
  failures.push(`${paths.topicInline}: topic invalidation must precede commit`);
}

requireAll(forumInbox, [
  '("search", _) | ("forum", _) | ("forum_topic", Some(_)) => Some(Self::Full)',
], `${paths.forumInbox}: topic reindex scope`);
const refreshEntity = functionBody(forumProjector, "refresh_entity");
requireAll(refreshEntity, [
  "if entity_type == FORUM_TOPIC_ENTITY_TYPE",
  "return self.rebuild_tenant(tenant_id).await;",
], `${paths.forumProjector}: parent-derived reply refresh`);

requireAll(searchLib, ["mod forum_current_channel_filter;"], paths.searchLib);
requireAll(execution, [
  "pub current_channel_only: Option<bool>",
  "current_channel_filter: ForumStorefrontCurrentChannelFilter",
  "resolve_current_channel_filter(request.current_channel_only, &trusted_channel)",
  "current_channel_only requires a trusted storefront channel",
  "document_filters.matches(item) && current_channel_filter.matches(item)",
  "document_filters.is_empty() && current_channel_filter.is_empty()",
  "let raw_total =",
  "let candidates = all_items",
  "let total = visible_items.len() as u64",
  "current_channel_only_rejects_unscoped_request",
], paths.execution);
if (execution.indexOf("let raw_total =") > execution.indexOf("current_channel_filter.matches(item)")) {
  failures.push(`${paths.execution}: raw candidate bound must precede channel narrowing`);
}
if (execution.indexOf("current_channel_filter.matches(item)") > execution.indexOf("let candidates = all_items")) {
  failures.push(`${paths.execution}: channel narrowing must precede owner eligibility candidates`);
}

requireAll(graphqlOwner, [
  "current_channel_only: Option<bool>",
  "current_channel_only,",
  "trusted current-channel scope",
], paths.graphqlOwner);
requireAll(graphqlAdapter, [
  "ForumStorefrontSearchByCurrentChannel",
  "currentChannelOnly: true",
  "fetch_search_with_current_channel",
], paths.graphqlAdapter);
requireAll(nativeAdapter, [
  'endpoint = "search/forum-storefront-search-by-current-channel"',
  "fetch_search_with_current_channel",
  "Some(true)",
  "current_channel_only: Option<bool>",
], paths.nativeAdapter);
requireAll(transportFacade, [
  "pub async fn fetch_forum_search_with_current_channel",
  "forum_native_server_adapter::fetch_search_with_current_channel",
  "forum_graphql_adapter::fetch_search_with_current_channel",
], paths.transportFacade);

requireAll(graphqlAdapter, [
  "ForumStorefrontSearch($input: SearchPreviewInput!)",
  "ForumStorefrontSearchByAuthors",
  "ForumStorefrontSearchByFilters",
  "ForumStorefrontSearchByDateWindow",
], `${paths.graphqlAdapter} legacy operations`);
requireAll(nativeAdapter, [
  'endpoint = "search/forum-storefront-search"',
  'endpoint = "search/forum-storefront-search-by-authors"',
  'endpoint = "search/forum-storefront-search-by-filters"',
  'endpoint = "search/forum-storefront-search-by-date-window"',
], `${paths.nativeAdapter} legacy endpoints`);

rejectAll(graphqlTypes, ["current_channel_only", "currentChannelOnly"], paths.graphqlTypes);
rejectAll(storefrontModel, ["current_channel_only", "currentChannelOnly"], paths.storefrontModel);
rejectAll(engine, [
  "current_channel_only",
  "currentChannelOnly",
  "ForumStorefrontCurrentChannelFilter",
], paths.engine);
rejectAll(
  [execution, filter, graphqlOwner, graphqlAdapter, nativeAdapter, transportFacade].join("\n"),
  ["group_ids", "groupIds", "requested_channel_slug", "requestedChannelSlug"],
  "current-channel boundary",
);

requireAll(note, [
  "# FORUM-23B2F4 trusted current-channel Search filter",
  "boolean, not a caller-selected channel slug",
  "Parent-derived refresh ordering",
  "update_with_inline_relations",
  "full channel/group roadmap therefore remains open",
  "did not run these commands",
], paths.note);

if (contract) {
  if (contract.task !== "FORUM-23B2F4") failures.push(`${paths.contract}: unexpected task`);
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${paths.contract}: unexpected status`);
  }
  if (contract.input?.arbitrary_channel_slug_input_allowed !== false) {
    failures.push(`${paths.contract}: arbitrary channel selection must remain forbidden`);
  }
  if (!contract.evaluation?.raw_candidate_limit_checked_before_channel_narrowing) {
    failures.push(`${paths.contract}: raw ordering invariant missing`);
  }
  if (contract.transport_compatibility?.existing_wire_signatures_changed !== false) {
    failures.push(`${paths.contract}: legacy wire signatures changed`);
  }
  if (contract.compatibility?.neutral_search_query_changed !== false) {
    failures.push(`${paths.contract}: neutral SearchQuery changed`);
  }
  if (!contract.projection_refresh?.topic_update_invalidation_is_transactional) {
    failures.push(`${paths.contract}: transactional topic invalidation missing`);
  }
  if (contract.projection_refresh?.topic_reindex_inbox_scope !== "Full") {
    failures.push(`${paths.contract}: topic reindex must use full scope`);
  }
  if (contract.projection_refresh?.topic_refresh_operation !== "rebuild_tenant") {
    failures.push(`${paths.contract}: topic refresh must rebuild the tenant projection`);
  }
  if (contract.projection_refresh?.legacy_update_with_relations_used_by_public_owner !== false) {
    failures.push(`${paths.contract}: public owner still permits legacy update path`);
  }
}

if (failures.length > 0) {
  console.error("FORUM-23B2F4 current-channel Search verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("FORUM-23B2F4 current-channel Search source contract is consistent.");
