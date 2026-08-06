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
  graphql: "crates/rustok-forum/src/graphql/topic_route_query.rs",
  graphqlMod: "crates/rustok-forum/src/graphql/mod.rs",
  servicesMod: "crates/rustok-forum/src/services/mod.rs",
  owner: "crates/rustok-forum/src/services/topic_route_tombstone_visibility.rs",
  model: "crates/rustok-forum/storefront/src/model.rs",
  graphqlAdapter:
    "crates/rustok-forum/storefront/src/transport/topic_route_graphql_adapter.rs",
  nativeAdapter:
    "crates/rustok-forum/storefront/src/transport/native_server_adapter_topic_route.rs",
  host: "apps/storefront/src/forum_topic_route.rs",
  hostLib: "apps/storefront/src/lib.rs",
  contract:
    "crates/rustok-forum/contracts/forum-topic-route-authorized-gone-transport.json",
  test: "crates/rustok-forum/tests/topic_route_authorized_gone_transport_contract.rs",
  docs: "crates/rustok-forum/docs/forum-24k-topic-route-authorized-gone.md",
};

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireText(content, marker, label) {
  if (!content.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function forbidText(content, marker, label) {
  if (content.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

const source = Object.fromEntries(
  Object.entries(paths).map(([key, value]) => [key, read(value)]),
);

let contract = null;
try {
  contract = JSON.parse(source.contract);
} catch (error) {
  failures.push(`${paths.contract}: invalid JSON (${error.message})`);
}

for (const marker of [
  "async fn forum_storefront_topic_route(",
  "map_legacy_public_route_resolution(resolution)",
  "ForumTopicRouteDisposition::Gone => return Ok(None)",
  "async fn forum_storefront_topic_route_decision(",
  "GqlForumStorefrontTopicRouteDecisionDisposition::Gone",
  "pub canonical: Option<GqlForumTopicRouteDescriptor>",
  "ForumTopicRouteTombstoneVisibilityService::new(db.clone())",
  ".can_disclose_public_gone(",
  "public_channel_slug(ctx).as_deref()",
  "if !channel_enabled",
]) {
  requireText(source.graphql, marker, paths.graphql);
}

for (const marker of [
  "GqlForumStorefrontTopicRouteDecision",
  "GqlForumStorefrontTopicRouteDecisionDisposition",
]) {
  requireText(source.graphqlMod, marker, paths.graphqlMod);
}
requireText(
  source.servicesMod,
  "ForumTopicRouteTombstoneVisibilityService",
  paths.servicesMod,
);
requireText(
  source.owner,
  "pub async fn can_disclose_public_gone(",
  paths.owner,
);

for (const marker of [
  "Gone",
  "pub canonical: Option<StorefrontForumTopicRouteDescriptor>",
]) {
  requireText(source.model, marker, paths.model);
}
for (const marker of [
  "forumStorefrontTopicRouteDecision",
  "StorefrontForumTopicRouteDecision",
]) {
  requireText(source.graphqlAdapter, marker, paths.graphqlAdapter);
}
for (const marker of [
  "ForumTopicRouteTombstoneVisibilityService",
  ".can_disclose_public_gone(",
  "request.channel_slug.as_deref()",
  "StorefrontForumTopicRouteDisposition::Gone",
  "(StorefrontForumTopicRouteDisposition::Gone, None)",
]) {
  requireText(source.nativeAdapter, marker, paths.nativeAdapter);
}
for (const marker of [
  "ForumTopicHostAction::Gone",
  "StatusCode::GONE",
  "This Forum topic route is no longer available",
  "ForumTopicHostAction::Invalid",
  "StatusCode::SERVICE_UNAVAILABLE",
]) {
  requireText(source.host, marker, paths.host);
}
for (const marker of [
  "const PRIVATE_NO_STORE: &str = \"private, no-store\"",
  "fn private_status_response",
]) {
  requireText(source.hostLib, marker, paths.hostLib);
}

for (const [label, content] of [
  [paths.graphql, source.graphql],
  [paths.nativeAdapter, source.nativeAdapter],
  [paths.host, source.host],
]) {
  for (const marker of [
    "forum_topic_route_tombstone_visibility",
    "forum_topic_route_tombstone_channels",
    "forum_topic_route_aliases",
    "SELECT ",
  ]) {
    forbidText(content, marker, label);
  }
}
for (const marker of [
  "ForumTopicRouteTombstoneVisibilityService",
  "can_disclose_public_gone",
]) {
  forbidText(source.host, marker, paths.host);
}
for (const marker of [
  "StoredForumTopicRouteTombstoneVisibility",
  "route_channel_digest",
  "load_snapshot_channel_slugs",
]) {
  forbidText(source.servicesMod, marker, paths.servicesMod);
}

for (const marker of [
  "legacy",
  "forumStorefrontTopicRouteDecision",
  "private `410 Gone`",
  "Authentication never broadens",
  "No tests, verifiers, formatting, Cargo commands",
]) {
  requireText(source.docs, marker, paths.docs);
}
for (const marker of [
  "legacy_field_changed",
  "decision_field_additive",
  "gone_requires_deletion_time_public_snapshot",
  "authenticated_requests_do_not_broaden_public_snapshot",
  "authorized_gone_status",
  "malformed_shapes_fail_closed",
]) {
  requireText(source.contract, marker, paths.contract);
}
for (const marker of [
  "legacy_field_still_hides_gone",
  "storefront_transports_share_terminal_decision_shape",
  "host_maps_only_authorized_terminal_decision_to_private_gone",
]) {
  requireText(source.test, marker, paths.test);
}

if (contract) {
  if (contract.task !== "FORUM-24K") {
    failures.push(`${paths.contract}: task must be FORUM-24K`);
  }
  if (contract.graphql?.legacy_field_changed !== false) {
    failures.push(`${paths.contract}: legacy GraphQL field must remain unchanged`);
  }
  if (contract.graphql?.decision_field_additive !== true) {
    failures.push(`${paths.contract}: decision field must be additive`);
  }
  if (contract.authorization?.authenticated_requests_do_not_broaden_public_snapshot !== true) {
    failures.push(`${paths.contract}: authentication must not broaden tombstone disclosure`);
  }
  if (contract.host?.authorized_gone_status !== 410) {
    failures.push(`${paths.contract}: authorized gone status must be 410`);
  }
  if (contract.host?.authorized_gone_cache_control !== "private, no-store") {
    failures.push(`${paths.contract}: authorized gone cache policy must be private, no-store`);
  }
  if (contract.shape_invariants?.malformed_shapes_fail_closed !== true) {
    failures.push(`${paths.contract}: malformed shapes must fail closed`);
  }
}

if (failures.length > 0) {
  console.error("forum authorized topic route gone verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum authorized topic route gone verification passed");
