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
  core: "crates/rustok-forum/storefront/src/core.rs",
  model: "crates/rustok-forum/storefront/src/model.rs",
  packageLib: "crates/rustok-forum/storefront/src/lib.rs",
  transport: "crates/rustok-forum/storefront/src/transport/mod.rs",
  graphql: "crates/rustok-forum/storefront/src/transport/topic_route_graphql_adapter.rs",
  native: "crates/rustok-forum/storefront/src/transport/native_server_adapter_topic_route.rs",
  host: "apps/storefront/src/forum_topic_route.rs",
  hostLib: "apps/storefront/src/lib.rs",
  contract: "crates/rustok-forum/contracts/forum-topic-route-storefront-mount.json",
  docs: "crates/rustok-forum/docs/forum-24i-topic-route-storefront-mount.md",
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
  "pub fn topic_href(topic_id: &str, locale: &str, slug: &str) -> Option<String>",
  "Uuid::parse_str(topic_id.trim())",
  "FORUM_TOPIC_ROUTE_SHORT_ID_LEN: usize = 12",
  "Some(format!(\"/{locale}/forum/t/{short_id}/{slug}\"))",
]) {
  requireText(source.core, marker, paths.core);
}
forbidText(source.core, "?topic={topic_id}", paths.core);

for (const marker of [
  "pub enum StorefrontForumTopicRouteDisposition",
  "Canonical",
  "Redirect",
  "Gone",
  "pub canonical: Option<StorefrontForumTopicRouteDescriptor>",
]) {
  requireText(source.model, marker, paths.model);
}

for (const marker of [
  "pub async fn resolve_storefront_topic_route(",
  "if use_native_transport()",
  "resolve_storefront_topic_route_server",
  "resolve_storefront_topic_route_graphql",
]) {
  requireText(source.transport, marker, paths.transport);
}

for (const marker of [
  "forumStorefrontTopicRouteDecision",
  "requestedLocale requestedShortId requestedSlug disposition canonical",
  "resolve_storefront_topic_route_graphql",
]) {
  requireText(source.graphql, marker, paths.graphql);
}
forbidText(source.graphql, "reqwest", paths.graphql);

for (const marker of [
  "endpoint = \"forum/storefront-topic-route\"",
  "ForumTopicRouteService::new(db.clone())",
  "ForumTopicAudienceReadService::with_audience_facts",
  "ForumTopicRouteTombstoneVisibilityService",
  ".can_disclose_public_gone(",
  "request.channel_slug.as_deref()",
  "StorefrontForumTopicRouteDisposition::Gone",
]) {
  requireText(source.native, marker, paths.native);
}
for (const marker of [
  "forum_topic_route_aliases",
  "forum_topic_route_tombstone_visibility",
  "Statement::from_sql_and_values",
  "SELECT ",
]) {
  forbidText(source.native, marker, paths.native);
}

for (const marker of [
  "pub use transport::{TransportError, resolve_storefront_topic_route}",
  "StorefrontForumTopicRouteDisposition",
  "StorefrontForumTopicRouteResolution",
]) {
  requireText(source.packageLib, marker, paths.packageLib);
}

for (const marker of [
  "rustok_forum_storefront::resolve_storefront_topic_route(",
  "ForumTopicHostAction::Gone",
  "StatusCode::GONE",
  "ForumTopicHostAction::Invalid",
  "StatusCode::SERVICE_UNAVAILABLE",
  "StatusCode::NOT_FOUND",
  "private_permanent_redirect(location.as_str())",
  "query_params.insert(\"topic\".to_string(), topic_id)",
]) {
  requireText(source.host, marker, paths.host);
}
for (const marker of [
  "ForumTopicRouteService",
  "ForumTopicAudienceReadService",
  "ForumTopicRouteTombstoneVisibilityService",
  "can_disclose_public_gone",
  "forum_topic_route_aliases",
  "forum_topic_route_tombstone_visibility",
]) {
  forbidText(source.host, marker, paths.host);
}

for (const marker of [
  "mod forum_topic_route;",
  "\"/{locale}/forum/t/{short_id}/{slug}\"",
  "original_uri: axum::extract::OriginalUri",
  "original_uri.0.path().to_string()",
  "const PRIVATE_NO_STORE: &str = \"private, no-store\"",
]) {
  requireText(source.hostLib, marker, paths.hostLib);
}

for (const marker of [
  "private `308 Permanent Redirect`",
  "private `410 Gone`",
  "private `404`",
  "private `503`",
  "There is no automatic fallback",
  "host still does not authorize `GONE`",
  "No tests, verifiers, formatting, Cargo commands",
]) {
  requireText(source.docs, marker, paths.docs);
}

if (contract) {
  if (contract.task !== "FORUM-24I") {
    failures.push(`${paths.contract}: task must remain FORUM-24I`);
  }
  if (contract.transport?.automatic_fallback !== false) {
    failures.push(`${paths.contract}: automatic fallback must remain disabled`);
  }
  if (contract.host_composition?.host_reads_forum_storage !== false) {
    failures.push(`${paths.contract}: host must not read Forum storage`);
  }
  if (contract.host_composition?.host_reauthorizes_gone !== false) {
    failures.push(`${paths.contract}: host must not reauthorize gone`);
  }
  if (contract.disclosure?.authorized_gone_exposed !== true) {
    failures.push(`${paths.contract}: FORUM-24K authorized gone extension is missing`);
  }
  if (contract.disclosure?.authenticated_requests_broaden_public_snapshot !== false) {
    failures.push(`${paths.contract}: authentication must not broaden snapshot disclosure`);
  }
  if (contract.compatibility?.seo_changed !== false) {
    failures.push(`${paths.contract}: SEO must remain out of scope`);
  }
}

if (failures.length > 0) {
  console.error("forum storefront topic route mount verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum storefront topic route mount verification passed");
