#!/usr/bin/env node
import { existsSync, readFileSync } from "node:fs";

const files = {
  lib: "crates/rustok-blog/storefront/src/lib.rs",
  core: "crates/rustok-blog/storefront/src/core.rs",
  pagination: "crates/rustok-blog/storefront/src/comments_pagination.rs",
  model: "crates/rustok-blog/storefront/src/model.rs",
  ui: "crates/rustok-blog/storefront/src/ui/leptos.rs",
  transport: "crates/rustok-blog/storefront/src/transport/mod.rs",
  native: "crates/rustok-blog/storefront/src/transport/native_server_adapter.rs",
  graphql: "crates/rustok-blog/storefront/src/transport/graphql_adapter.rs",
  graphqlTypes: "crates/rustok-blog/src/graphql/types.rs",
  cargo: "crates/rustok-blog/storefront/Cargo.toml",
  legacyApi: "crates/rustok-blog/storefront/src/api.rs",
  plan: "crates/rustok-blog/docs/implementation-plan.md",
  registry: "docs/modules/registry.md",
  packageJson: "package.json",
  verifierTest: "scripts/verify/verify-blog-storefront-boundary.test.mjs",
};

function fail(message) {
  console.error("blog storefront boundary verification failed:");
  console.error(`- ${message}`);
  process.exit(1);
}

function text(path) {
  try {
    return readFileSync(path, "utf8");
  } catch (error) {
    fail(`${path}: ${error.message}`);
  }
}

function assertContains(source, needle, message) {
  if (!source.includes(needle)) fail(message);
}

function assertNotContains(source, needle, message) {
  if (source.includes(needle)) fail(message);
}

const lib = text(files.lib);
const core = text(files.core);
const pagination = text(files.pagination);
const model = text(files.model);
const ui = text(files.ui);
const transport = text(files.transport);
const native = text(files.native);
const graphql = text(files.graphql);
const graphqlTypes = text(files.graphqlTypes);
const cargo = text(files.cargo);
const plan = text(files.plan);
const registry = text(files.registry);
const verifierTest = text(files.verifierTest);
const pkg = JSON.parse(text(files.packageJson));

if (existsSync(files.legacyApi)) {
  fail(`${files.legacyApi}: legacy api.rs must stay removed; transport adapters own native/GraphQL endpoints`);
}

assertNotContains(lib, "mod api;", `${files.lib}: lib must not wire legacy api module`);
assertContains(lib, "mod comments_pagination;", `${files.lib}: lib must wire route-owned comment pagination policy`);
assertContains(lib, "mod transport;", `${files.lib}: lib must wire transport facade`);
assertContains(lib, "pub use ui::BlogView", `${files.lib}: lib must only re-export BlogView`);

for (const marker of ["leptos::", "view!", "#[server", "ServerFnError"]) {
  assertNotContains(core, marker, `${files.core}: core must remain framework/server-function free (${marker})`);
  assertNotContains(pagination, marker, `${files.pagination}: pagination policy must remain framework/server-function free (${marker})`);
}
for (const marker of [
  "COMMENTS_PAGE_QUERY_KEY: &str = \"commentsPage\"",
  "COMMENTS_PAGE_SIZE: u64 = 20",
  "MAX_COMMENTS_PAGE",
  "bounded_comments_request_page",
  "comments_page_from_query",
  "comments_total_pages",
  "comments_page_query_intent",
  "UiRouteQueryIntent::clear(COMMENTS_PAGE_QUERY_KEY)",
]) {
  assertContains(pagination, marker, `${files.pagination}: missing pagination policy marker ${marker}`);
}

assertContains(model, "pub struct BlogCommentList", `${files.model}: storefront DTO must model the public comment list`);
assertContains(model, "pub public_comments: BlogCommentList", `${files.model}: selected posts must carry public comments`);
assertContains(ui, "use_route_query_value(comments_pagination::COMMENTS_PAGE_QUERY_KEY)", `${files.ui}: UI must read route-owned comments page state`);
assertContains(ui, "use_route_query_writer()", `${files.ui}: UI must write pagination intents through shared routing`);
assertContains(ui, "transport::fetch_blog(request, comments_page)", `${files.ui}: UI must pass the current comments page through transport`);
assertContains(ui, "<PublicCommentsList comments=public_comments comments_page />", `${files.ui}: selected post must render paginated public comments`);
assertContains(ui, "comments_pagination::comments_page_query_intent", `${files.ui}: pagination controls must use the pure route policy`);
assertNotContains(ui, "crate::api", `${files.ui}: UI must not call legacy api module`);

assertContains(transport, "pub mod graphql_adapter;", `${files.transport}: transport facade must wire GraphQL adapter`);
assertContains(transport, "pub mod native_server_adapter;", `${files.transport}: transport facade must wire native adapter`);
assertContains(transport, "comments_page: u64", `${files.transport}: transport facade must carry comments page`);
assertContains(transport, "native_server_adapter::fetch_blog(native_request, comments_page)", `${files.transport}: native path must receive comments page`);
assertContains(transport, "graphql_adapter::fetch_blog(request, comments_page)", `${files.transport}: GraphQL path must receive comments page`);
assertNotContains(transport, "crate::api", `${files.transport}: transport facade must not delegate to legacy api module`);

for (const marker of [
  "ChannelService::new",
  ".is_module_enabled(channel_id, MODULE_SLUG)",
  "normalize_channel_slug",
  "is_visible_for_public_channel",
  "request_context.channel_slug",
  "Module '{MODULE_SLUG}' is not enabled for channel",
  "CommentService::new",
  ".list_for_post_with_locale_fallback(",
  "SecurityContext::public_read()",
  "page: comments_page.max(1)",
  "per_page: COMMENTS_PAGE_SIZE",
  "map_comment_list_item",
]) {
  assertContains(native, marker, `${files.native}: missing channel/public-comments native marker ${marker}`);
}
assertContains(native, "#[server(prefix = \"/api/fn\", endpoint = \"blog/storefront-data\")]", `${files.native}: native adapter must own server function endpoint`);
assertContains(native, "expect_context::<HostRuntimeContext>()", `${files.native}: native adapter must use the host runtime context`);
assertContains(native, "shared_get::<TransactionalEventBus>()", `${files.native}: native adapter must receive the event bus through the host runtime context`);
assertContains(native, "runtime_ctx.db_clone()", `${files.native}: native adapter must receive DB through the host runtime context`);

assertContains(graphql, "GraphqlRequest", `${files.graphql}: GraphQL adapter must keep GraphQL request contract`);
assertContains(graphql, "STOREFRONT_BLOG_QUERY", `${files.graphql}: GraphQL adapter must own storefront blog query`);
assertContains(graphql, "$commentsPage: Int!", `${files.graphql}: GraphQL query must declare comments page`);
assertContains(graphql, "$commentsPerPage: Int!", `${files.graphql}: GraphQL query must declare comments page size`);
assertContains(graphql, "publicComments(locale: $locale, page: $commentsPage, perPage: $commentsPerPage)", `${files.graphql}: GraphQL storefront query must request the selected comments page`);
assertContains(graphql, "bounded_comments_request_page(comments_page)", `${files.graphql}: GraphQL page input must be bounded before serialization`);
assertContains(graphql, "comments_per_page: COMMENTS_PAGE_SIZE", `${files.graphql}: GraphQL and native page size must match`);
for (const marker of [
  "#[graphql(complex)]",
  "async fn public_comments(",
  "CommentService::new",
  "SecurityContext::public_read()",
  "GqlPublicCommentList",
]) {
  assertContains(graphqlTypes, marker, `${files.graphqlTypes}: missing approved public comments GraphQL marker ${marker}`);
}

assertContains(plan, "verify-blog-storefront-boundary.mjs", `${files.plan}: local plan must mention storefront guardrail`);
assertContains(plan, "public comments", `${files.plan}: local plan must record public comment rendering parity`);
assertContains(plan, "storefront comment pagination", `${files.plan}: local plan must record route-owned comment pagination`);
assertContains(registry, "verify-blog-storefront-boundary.mjs", `${files.registry}: central board must mention storefront guardrail`);
assertContains(verifierTest, "passes canonical fixture", `${files.verifierTest}: fixture tests must cover canonical pass path`);
assertContains(verifierTest, "rejects legacy api module", `${files.verifierTest}: fixture tests must reject legacy api module`);
assertContains(verifierTest, "rejects missing public comments parity", `${files.verifierTest}: fixture tests must reject missing comments parity`);
assertContains(verifierTest, "rejects missing comment pagination parity", `${files.verifierTest}: fixture tests must reject missing pagination parity`);

const scripts = pkg.scripts ?? {};
if (scripts["verify:blog:storefront-boundary"] !== "node scripts/verify/verify-blog-storefront-boundary.mjs") {
  fail(`${files.packageJson}: package scripts must expose blog storefront verifier`);
}
if (!String(scripts["verify:ffa:ui:migration"] ?? "").includes("npm run verify:blog:storefront-boundary")) {
  fail(`${files.packageJson}: aggregate FFA verifier must include blog storefront verifier`);
}
if (scripts["test:verify:blog:storefront-boundary"] !== "node scripts/verify/verify-blog-storefront-boundary.test.mjs") {
  fail(`${files.packageJson}: package scripts must expose blog storefront verifier tests`);
}
if (!String(scripts["test:verify:ffa:ui:migration"] ?? "").includes("npm run test:verify:blog:storefront-boundary")) {
  fail(`${files.packageJson}: aggregate FFA tests must include blog storefront verifier tests`);
}

console.log("blog storefront boundary verification passed");
