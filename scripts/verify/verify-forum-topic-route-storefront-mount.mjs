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
  graphqlOwner: "crates/rustok-forum/src/graphql/topic_route_query.rs",
  host: "apps/storefront/src/forum_topic_route.rs",
  hostLib: "apps/storefront/src/lib.rs",
  contract: "crates/rustok-forum/contracts/forum-topic-route-storefront-mount.json",
  docs: "crates/rustok-forum/docs/forum-24i-topic-route-storefront-mount.md",
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

const core = read(paths.core);
const model = read(paths.model);
const packageLib = read(paths.packageLib);
const transport = read(paths.transport);
const graphql = read(paths.graphql);
const native = read(paths.native);
const graphqlOwner = read(paths.graphqlOwner);
const host = read(paths.host);
const hostLib = read(paths.hostLib);
const contractText = read(paths.contract);
const docs = read(paths.docs);

let contract = null;
try {
  contract = JSON.parse(contractText);
} catch (error) {
  failures.push(`${paths.contract}: invalid JSON (${error.message})`);
}

for (const marker of [
  "pub fn topic_href(topic_id: &str, locale: &str, slug: &str) -> Option<String>",
  "Uuid::parse_str(topic_id.trim())",
  "FORUM_TOPIC_ROUTE_SHORT_ID_LEN: usize = 12",
  "Some(format!(\"/{locale}/forum/t/{short_id}/{slug}\"))",
  "item.effective_locale.as_str()",
  "item.slug.as_str()",
]) {
  requireText(core, marker, paths.core);
}
forbidText(core, "&topic={topic_id}", paths.core);
forbidText(core, "?topic={topic_id}", paths.core);

for (const marker of [
  "pub enum StorefrontForumTopicRouteDisposition",
  "Canonical",
  "Redirect",
  "pub struct StorefrontForumTopicRouteResolution",
  "pub canonical: StorefrontForumTopicRouteDescriptor",
]) {
  requireText(model, marker, paths.model);
}

for (const marker of [
  "include!(\"topic_route_graphql_adapter.rs\")",
  "include!(\"native_server_adapter_topic_route.rs\")",
  "pub async fn resolve_storefront_topic_route(",
  "if use_native_transport()",
  "resolve_storefront_topic_route_server",
  "resolve_storefront_topic_route_graphql",
]) {
  requireText(transport, marker, paths.transport);
}

for (const marker of [
  "forumStorefrontTopicRoute",
  "requestedLocale requestedShortId requestedSlug disposition canonical",
  "tenant_id: None",
  "resolve_storefront_topic_route_graphql",
]) {
  requireText(graphql, marker, paths.graphql);
}
forbidText(graphql, "fetch(", paths.graphql);
forbidText(graphql, "reqwest", paths.graphql);

for (const marker of [
  "endpoint = \"forum/storefront-topic-route\"",
  "ForumTopicRouteService::new(db.clone())",
  ".resolve(tenant.id, &locale, &short_id, &slug)",
  "ForumTopicAudienceReadService::with_audience_facts",
  "ForumTopicAudienceReadService::new",
  "topic_read_audience_port_context(",
  "ForumTopicReadTransport::NativeServer",
  "ForumTopicReadOperation::SelectedTopic",
  ".get_authenticated_storefront_visible_with_audience_context(",
  ".get_public_storefront_visible_with_locale_fallback(",
  "ForumTopicRouteDisposition::Gone => return Ok(None)",
  "ChannelService::new(db.clone())",
]) {
  requireText(native, marker, paths.native);
}
for (const marker of [
  "forum_topic_route_aliases",
  "Statement::from_sql_and_values",
  "SELECT ",
  "record_redirect_alias",
  "record_gone_alias",
]) {
  forbidText(native, marker, paths.native);
}

for (const marker of [
  ".topic_audience_read_service(db.clone(), event_bus.clone())",
  "ForumTopicReadTransport::Graphql",
  "ForumTopicReadOperation::SelectedTopic",
  ".get_authenticated_storefront_visible_with_audience_context(",
  ".get_public_storefront_visible_with_locale_fallback(",
]) {
  requireText(graphqlOwner, marker, paths.graphqlOwner);
}

for (const marker of [
  "pub use transport::{TransportError, resolve_storefront_topic_route}",
  "StorefrontForumTopicRouteDisposition",
  "StorefrontForumTopicRouteResolution",
]) {
  requireText(packageLib, marker, paths.packageLib);
}

for (const marker of [
  "requested_path: String",
  "rustok_forum_storefront::resolve_storefront_topic_route(",
  "StorefrontForumTopicRouteDisposition::Redirect",
  "requested_path != canonical_path",
  "private_permanent_redirect(location.as_str())",
  "StatusCode::NOT_FOUND",
  "StatusCode::SERVICE_UNAVAILABLE",
  "query_params.insert(\"topic\".to_string(), topic_id)",
  "render_module_page_with_nonce(",
  "FORUM_ROUTE_SEGMENT",
]) {
  requireText(host, marker, paths.host);
}
for (const marker of [
  "let requested_path = format!",
  "ForumTopicRouteService",
  "ForumTopicAudienceReadService",
  "forum_topic_route_aliases",
  "Uuid::parse_str",
  "record_redirect_alias",
  "record_gone_alias",
]) {
  forbidText(host, marker, paths.host);
}

for (const marker of [
  "mod forum_topic_route;",
  "\"/{locale}/forum/t/{short_id}/{slug}\"",
  "original_uri: axum::extract::OriginalUri",
  "original_uri.0.path().to_string()",
  "forum_topic_route::render_forum_topic_route_response(",
]) {
  requireText(hostLib, marker, paths.hostLib);
}

for (const marker of [
  "private `308 Permanent Redirect`",
  "private `404 Not Found`",
  "private `503 Service Unavailable`",
  "There is no automatic fallback",
  "does not add:",
  "public `410 Gone`",
  "No tests, verifiers, formatting, Cargo commands",
]) {
  requireText(docs, marker, paths.docs);
}

if (contract) {
  if (contract.task !== "FORUM-24I") {
    failures.push(`${paths.contract}: task must be FORUM-24I`);
  }
  if (contract.status !== "source_ready_maintainer_execution_pending") {
    failures.push(`${paths.contract}: unexpected source status`);
  }
  if (contract.canonical_route !== "/{locale}/forum/t/{short_id}/{slug}") {
    failures.push(`${paths.contract}: unexpected canonical route`);
  }
  if (contract.transport?.automatic_fallback !== false) {
    failures.push(`${paths.contract}: automatic fallback must remain disabled`);
  }
  if (contract.transport?.exact_audience_recheck_both_paths !== true) {
    failures.push(`${paths.contract}: exact audience parity is required`);
  }
  if (contract.topic_card_cutover?.legacy_topic_query_links_emitted !== false) {
    failures.push(`${paths.contract}: legacy topic query links must stay disabled`);
  }
  if (contract.host_composition?.host_reads_forum_storage !== false) {
    failures.push(`${paths.contract}: host must not read Forum storage`);
  }
  if (contract.host_composition?.host_recomputes_redirect_target !== false) {
    failures.push(`${paths.contract}: host must trust the owner canonical target`);
  }
  if (contract.disclosure?.gone_exposed !== false) {
    failures.push(`${paths.contract}: public gone must remain hidden`);
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
