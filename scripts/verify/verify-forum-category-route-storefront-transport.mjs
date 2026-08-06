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
  graphql: "crates/rustok-forum/src/graphql/category_route_query.rs",
  graphqlMod: "crates/rustok-forum/src/graphql/mod.rs",
  routeOwner: "crates/rustok-forum/src/services/category_route.rs",
  audienceOwner: "crates/rustok-forum/src/services/category_audience_read.rs",
  model: "crates/rustok-forum/storefront/src/model.rs",
  graphqlAdapter:
    "crates/rustok-forum/storefront/src/transport/category_route_graphql_adapter.rs",
  nativeAdapter:
    "crates/rustok-forum/storefront/src/transport/native_server_adapter_category_route.rs",
  transportMod: "crates/rustok-forum/storefront/src/transport/mod.rs",
  storefrontLib: "crates/rustok-forum/storefront/src/lib.rs",
  contract:
    "crates/rustok-forum/contracts/forum-category-route-storefront-transport.json",
  contractTest:
    "crates/rustok-forum/tests/category_route_storefront_transport_contract.rs",
  docs: "crates/rustok-forum/docs/forum-24n-category-route-storefront-transport.md",
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
  "pub(crate) struct ForumCategoryRouteQuery",
  "async fn forum_storefront_category_route(",
  "require_module_enabled(ctx, MODULE_SLUG)",
  "ForumCategoryRouteService::new(db.clone())",
  "Some(tenant.default_locale.as_str())",
  "if !forum_channel_enabled(ctx).await?",
  "runtime.category_audience_read_service(db.clone())",
  "ForumCategoryReadTransport::Graphql",
  "ForumCategoryReadOperation::SelectedCategory",
  "get_authenticated_storefront_visible_with_audience_context",
  "get_public_storefront_visible_with_locale_fallback",
  "GqlForumStorefrontCategoryRouteDisposition",
  "GqlForumStorefrontCategoryRouteDescriptor",
  "GqlForumStorefrontCategoryRouteResolution",
]) {
  requireText(source.graphql, marker, paths.graphql);
}

for (const marker of [
  "mod category_route_query;",
  "category_route_query::ForumCategoryRouteQuery",
  "GqlForumStorefrontCategoryRouteDescriptor",
  "GqlForumStorefrontCategoryRouteDisposition",
  "GqlForumStorefrontCategoryRouteResolution",
]) {
  requireText(source.graphqlMod, marker, paths.graphqlMod);
}

for (const marker of [
  "pub struct ForumCategoryRouteService",
  "pub async fn resolve(",
  "pub alias_id: Option<Uuid>",
  "Exact-locale aliases therefore precede fallback-locale current",
]) {
  requireText(source.routeOwner, marker, paths.routeOwner);
}

for (const marker of [
  "pub struct ForumCategoryAudienceReadService",
  "get_authenticated_storefront_visible_with_audience_context",
  "get_public_storefront_visible_with_locale_fallback",
]) {
  requireText(source.audienceOwner, marker, paths.audienceOwner);
}

for (const marker of [
  "StorefrontForumCategoryRouteDisposition",
  "StorefrontForumCategoryRouteDescriptor",
  "StorefrontForumCategoryRouteResolution",
  "category_route_payload_uses_graphql_enum_and_field_names",
]) {
  requireText(source.model, marker, paths.model);
}

for (const marker of [
  "forumStorefrontCategoryRoute",
  "requestedLocale requestedSlug disposition",
  "categoryId locale slug path",
  "resolve_storefront_category_route_graphql",
]) {
  requireText(source.graphqlAdapter, marker, paths.graphqlAdapter);
}

for (const marker of [
  "endpoint = \"forum/storefront-category-route\"",
  "expect_context::<HostRuntimeContext>()",
  "extract::<TenantContext>()",
  "extract::<OptionalAuthContext>()",
  "extract::<RequestContext>()",
  "is_module_enabled(channel_id, \"forum\")",
  "ForumCategoryRouteService::new(db.clone())",
  "ForumCategoryAudienceReadService::with_audience_facts",
  "ForumCategoryReadTransport::NativeServer",
  "ForumCategoryReadOperation::SelectedCategory",
  "get_authenticated_storefront_visible_with_audience_context",
  "get_public_storefront_visible_with_locale_fallback",
  "map_native_category_route_resolution",
]) {
  requireText(source.nativeAdapter, marker, paths.nativeAdapter);
}

for (const marker of [
  "include!(\"category_route_graphql_adapter.rs\")",
  "include!(\"native_server_adapter_category_route.rs\")",
  "pub async fn resolve_storefront_category_route(",
  "resolve_storefront_category_route_server",
  "resolve_storefront_category_route_graphql",
]) {
  requireText(source.transportMod, marker, paths.transportMod);
}

for (const marker of [
  "StorefrontForumCategoryRouteDescriptor",
  "StorefrontForumCategoryRouteDisposition",
  "StorefrontForumCategoryRouteResolution",
  "resolve_storefront_category_route",
]) {
  requireText(source.storefrontLib, marker, paths.storefrontLib);
}

for (const marker of [
  "graphql_route_resolution_rechecks_exact_category_visibility",
  "native_route_resolution_uses_trusted_context_and_same_owners",
  "public_dto_and_adapters_have_graphql_native_parity",
  "transport_slice_does_not_mount_or_add_seo_policy",
]) {
  requireText(source.contractTest, marker, paths.contractTest);
}

for (const marker of [
  "source-ready / maintainer execution pending",
  "Alias ownership is not visibility authorization",
  "No host router or UI invokes it in this slice",
  "No tests, verifiers, formatting, Cargo commands",
]) {
  requireText(source.docs, marker, paths.docs);
}

for (const marker of [
  "alias_id: Option",
  "alias_reason",
  "GqlForumStorefrontCategoryRouteGone",
  "StatusCode::",
  "Redirect::",
  "hreflang",
  "schema.org",
]) {
  forbidText(source.graphql, marker, paths.graphql);
  forbidText(source.nativeAdapter, marker, paths.nativeAdapter);
  forbidText(source.graphqlAdapter, marker, paths.graphqlAdapter);
}

if (contract) {
  if (contract.task !== "FORUM-24N") {
    failures.push(`${paths.contract}: task must be FORUM-24N`);
  }
  if (contract.status !== "source_ready_maintainer_execution_pending") {
    failures.push(`${paths.contract}: unexpected source status`);
  }
  if (contract.graphql?.field !== "forumStorefrontCategoryRoute") {
    failures.push(`${paths.contract}: unexpected GraphQL field`);
  }
  if (contract.route?.gone_supported !== false) {
    failures.push(`${paths.contract}: category gone must remain unsupported`);
  }
  if (contract.route?.alias_id_exposed !== false) {
    failures.push(`${paths.contract}: alias id must remain private`);
  }
  if (contract.authorization?.canonical_category_rechecked_before_disclosure !== true) {
    failures.push(`${paths.contract}: exact visibility recheck is required`);
  }
  if (contract.authorization?.alias_existence_never_authorizes_disclosure !== true) {
    failures.push(`${paths.contract}: aliases must not authorize disclosure`);
  }
  if (contract.parity?.graphql_and_native_use_same_route_owner !== true) {
    failures.push(`${paths.contract}: route owner parity is required`);
  }
  if (contract.parity?.graphql_and_native_use_same_category_audience_owner !== true) {
    failures.push(`${paths.contract}: audience owner parity is required`);
  }
  if (contract.compatibility?.category_route_mounted_in_host !== false) {
    failures.push(`${paths.contract}: category host mount must remain out of scope`);
  }
  if (contract.compatibility?.http_status_mapping_changed !== false) {
    failures.push(`${paths.contract}: HTTP status mapping must remain unchanged`);
  }
  if (contract.compatibility?.seo_or_hreflang_changed !== false) {
    failures.push(`${paths.contract}: SEO and hreflang must remain unchanged`);
  }
  if (contract.verification?.executed_by_implementation_agent !== false) {
    failures.push(`${paths.contract}: execution must not be claimed`);
  }
}

if (failures.length > 0) {
  console.error("forum category route storefront transport verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum category route storefront transport verification passed");
