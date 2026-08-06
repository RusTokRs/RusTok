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
  "require_module_enabled(ctx, MODULE_SLUG).await?",
  "require_public_forum_channel_enabled(ctx).await?",
  "Permission denied: tenant scope mismatch",
  "ForumTopicRouteService::new(db.clone())",
  ".resolve(tenant_id, &locale, &short_id, &slug)",
  ".topic_audience_read_service(db.clone(), event_bus.clone())",
  "topic_read_audience_port_context(",
  "ForumTopicReadTransport::Graphql",
  "ForumTopicReadOperation::SelectedTopic",
  ".get_authenticated_storefront_visible_with_audience_context(",
  ".get_public_storefront_visible_with_locale_fallback(",
  "SecurityContext::from_permission_snapshot",
  "ForumTopicRouteDisposition::Gone => return Ok(None)",
  "GqlForumStorefrontTopicRouteDisposition::Canonical",
  "GqlForumStorefrontTopicRouteDisposition::Redirect",
]) {
  requireText(query, marker, paths.query);
}

for (const marker of [
  "TopicService::new",
  "is_topic_visible_for_channel(",
  "crate::constants::topic_status::OPEN",
  "pub requested_topic_id",
  "pub alias_id",
  "GqlForumStorefrontTopicRouteDisposition::Gone",
  "forum_topic_route_aliases",
  "Statement::from_sql_and_values",
  "record_redirect_alias_in_tx",
  "record_gone_alias_in_tx",
  "ForumTopicRouteService::short_identity",
]) {
  forbidText(query, marker, paths.query);
}

requireText(graphqlMod, "mod topic_route_query;", paths.graphqlMod);
requireText(
  graphqlMod,
  "topic_route_query::ForumTopicRouteQuery",
  paths.graphqlMod,
);
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
  "ForumTopicAudienceVisibilityService",
]) {
  requireText(audienceOwner, marker, paths.audienceOwner);
}

for (const marker of [
  "forumStorefrontTopicRoute",
  "GqlForumStorefrontTopicRouteResolution",
  "CANONICAL",
  "REDIRECT",
  "requestedLocale",
  "requestedShortId",
  "requestedSlug",
  ".topic_audience_read_service",
]) {
  requireText(test, marker, paths.test);
}

for (const marker of [
  "visibility snapshot",
  "returns `null`",
  "does not expose",
  "public `GONE`",
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
  if (contract.storefront_visibility_owner !== "ForumTopicAudienceReadService") {
    failures.push(`${paths.contract}: exact audience owner must be declared`);
  }
  if (contract.output?.gone_exposed !== false) {
    failures.push(`${paths.contract}: gone_exposed must remain false`);
  }
  if (contract.output?.requested_topic_id_exposed !== false) {
    failures.push(`${paths.contract}: requested topic identity must stay hidden`);
  }
  if (contract.output?.alias_id_exposed !== false) {
    failures.push(`${paths.contract}: alias identity must stay hidden`);
  }
  if (contract.authorization?.canonical_topic_rechecked !== true) {
    failures.push(`${paths.contract}: canonical topic visibility recheck is required`);
  }
  if (contract.authorization?.exact_audience_owner_rechecked !== true) {
    failures.push(`${paths.contract}: exact audience recheck is required`);
  }
  if (contract.route_policy?.canonical_resolution_reimplemented !== false) {
    failures.push(`${paths.contract}: canonical resolution must remain owner-defined`);
  }
}

if (failures.length > 0) {
  console.error("forum storefront topic route GraphQL verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum storefront topic route GraphQL verification passed");
