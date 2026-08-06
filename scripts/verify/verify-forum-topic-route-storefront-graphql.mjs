#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

const paths = {
  query: "crates/rustok-forum/src/graphql/topic_route_query.rs",
  graphqlMod: "crates/rustok-forum/src/graphql/mod.rs",
  owner: "crates/rustok-forum/src/services/topic_route.rs",
  audienceOwner: "crates/rustok-forum/src/services/topic_audience_read.rs",
  tombstoneOwner: "crates/rustok-forum/src/services/topic_route_tombstone_visibility.rs",
  contract: "crates/rustok-forum/contracts/forum-topic-route-storefront-graphql.json",
  test: "crates/rustok-forum/tests/topic_route_storefront_graphql_contract.rs",
  docs: "crates/rustok-forum/docs/forum-24h-topic-route-storefront-graphql.md",
};

function absolute(relativePath) {
  return path.join(repoRoot, relativePath);
}

function read(relativePath) {
  if (!existsSync(absolute(relativePath))) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(absolute(relativePath), "utf8");
}

function requireText(content, marker, label) {
  if (!content.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function forbidText(content, marker, label) {
  if (content.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

const query = read(paths.query);
const graphqlMod = read(paths.graphqlMod);
const owner = read(paths.owner);
const audienceOwner = read(paths.audienceOwner);
const tombstoneOwner = read(paths.tombstoneOwner);
const contractText = read(paths.contract);
const test = read(paths.test);
const docs = read(paths.docs);

let contract = null;
try {
  contract = JSON.parse(contractText);
} catch (error) {
  failures.push(`${paths.contract}: invalid JSON (${error.message})`);
}

for (const marker of [
  "async fn forum_storefront_topic_route(",
  "map_legacy_public_route_resolution(resolution)",
  "ForumTopicRouteDisposition::Gone => return Ok(None)",
  "async fn forum_storefront_topic_route_decision(",
  "ForumTopicRouteTombstoneVisibilityService::new(db.clone())",
  ".can_disclose_public_gone(",
  "GqlForumStorefrontTopicRouteDecisionDisposition::Gone",
  "pub canonical: Option<GqlForumTopicRouteDescriptor>",
  "require_module_enabled(ctx, MODULE_SLUG).await?",
  "forum_channel_enabled(ctx).await?",
  "Permission denied: tenant scope mismatch",
  "ForumTopicRouteService::new(db.clone())",
  ".topic_audience_read_service(db.clone(), event_bus.clone())",
  "topic_read_audience_port_context(",
  ".get_authenticated_storefront_visible_with_audience_context(",
  ".get_public_storefront_visible_with_locale_fallback(",
]) {
  requireText(query, marker, paths.query);
}

for (const marker of [
  "TopicService::new",
  "pub requested_topic_id",
  "pub alias_id",
  "GqlForumStorefrontTopicRouteDisposition::Gone",
  "forum_topic_route_aliases",
  "forum_topic_route_tombstone_visibility",
  "forum_topic_route_tombstone_channels",
  "Statement::from_sql_and_values",
  "record_redirect_alias_in_tx",
  "record_gone_alias_in_tx",
  "ForumTopicRouteService::short_identity",
]) {
  forbidText(query, marker, paths.query);
}

requireText(graphqlMod, "mod topic_route_query;", paths.graphqlMod);
requireText(graphqlMod, "topic_route_query::ForumTopicRouteQuery", paths.graphqlMod);
requireText(owner, "pub async fn resolve(", paths.owner);
requireText(
  owner,
  "Callers must perform the\n/// same visibility/read authorization required for the canonical topic",
  paths.owner,
);
for (const marker of [
  "pub struct ForumTopicAudienceReadService",
  "get_authenticated_storefront_visible_with_audience_context",
  "get_public_storefront_visible_with_locale_fallback",
]) {
  requireText(audienceOwner, marker, paths.audienceOwner);
}
requireText(
  tombstoneOwner,
  "pub async fn can_disclose_public_gone(",
  paths.tombstoneOwner,
);

for (const marker of [
  "forumStorefrontTopicRoute",
  "forumStorefrontTopicRouteDecision",
  "GqlForumStorefrontTopicRouteDecision",
  "GONE",
  ".can_disclose_public_gone(",
]) {
  requireText(test, marker, paths.test);
}

for (const marker of [
  "FORUM-24J",
  "FORUM-24K",
  "legacy",
  "does not expose",
  "No commands were executed",
]) {
  requireText(docs, marker, paths.docs);
}

if (contract) {
  if (contract.task !== "FORUM-24H") {
    failures.push(`${paths.contract}: task must be FORUM-24H`);
  }
  if (contract.status !== "source_ready_maintainer_execution_pending") {
    failures.push(`${paths.contract}: unexpected source status`);
  }
  if (contract.output?.gone_exposed !== false) {
    failures.push(`${paths.contract}: legacy gone_exposed must remain false`);
  }
  if (contract.output?.requested_topic_id_exposed !== false) {
    failures.push(`${paths.contract}: requested topic identity must stay hidden`);
  }
  if (contract.authorization?.canonical_topic_rechecked !== true) {
    failures.push(`${paths.contract}: canonical topic visibility recheck is required`);
  }
}

if (failures.length > 0) {
  console.error("forum storefront topic route GraphQL verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum storefront topic route GraphQL verification passed");
