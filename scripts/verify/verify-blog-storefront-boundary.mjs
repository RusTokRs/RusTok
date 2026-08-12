#!/usr/bin/env node
import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";

const files = {
  storefrontSrc: "crates/rustok-blog/storefront/src",
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
  evidence: "crates/rustok-blog/contracts/evidence/blog-storefront-richtext-view.json",
  plan: "crates/rustok-blog/docs/implementation-plan.md",
  registry: "docs/modules/registry.md",
  packageJson: "package.json",
  verifier: "scripts/verify/verify-blog-storefront-boundary.mjs",
  verifierTest: "scripts/verify/verify-blog-storefront-boundary.test.mjs",
};

const expectedScope = [
  files.core,
  files.model,
  files.graphql,
  files.native,
  files.ui,
];
const expectedContract = {
  graphql_owner_view: true,
  native_owner_view: true,
  server_html_render: true,
  plain_text_fallback: true,
  legacy_body_transport: false,
  legacy_body_format_transport: false,
  local_format_renderer: false,
  legacy_summarizer_removed: true,
};
const expectedCanonicalContract = {
  read: "rustok_api::RichTextView",
  html: "server-derived",
  plain_text: "server-derived",
};
const expectedRenderContract = {
  component: "SelectedPostCard",
  html_sink: "RichTextHtml(view=content,content_locale=effective_locale)",
  fallback_sink: "selected_post_content.body",
  forbidden_storefront_markers: [
    "content.document",
    "pulldown_cmark",
    "comrak::",
    "markdown_to_html",
    "render_richtext",
    "render_document",
  ],
};

const legacySummarizerMarkers = [
  "body_or_fallback",
  "summarized_body_or_fallback",
  "summarize_content",
];

function fail(message) {
  console.error("blog storefront boundary verification failed:");
  console.error(`- ${message}`);
  process.exit(1);
}

function text(filePath) {
  try {
    return readFileSync(filePath, "utf8");
  } catch (error) {
    fail(`${filePath}: ${error.message}`);
  }
}

function assertContains(source, needle, message) {
  if (!source.includes(needle)) fail(message);
}

function assertNotContains(source, needle, message) {
  if (source.includes(needle)) fail(message);
}

function between(source, start, end, filePath) {
  const from = source.indexOf(start);
  const to = source.indexOf(end, from + start.length);
  if (from === -1 || to === -1) {
    fail(`${filePath}: could not isolate ${start} before ${end}`);
  }
  return source.slice(from, to);
}

function rustFilesUnder(root) {
  return readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const entryPath = path.join(root, entry.name).replaceAll("\\", "/");
    if (entry.isDirectory()) return rustFilesUnder(entryPath);
    return entry.isFile() && entry.name.endsWith(".rs") ? [entryPath] : [];
  });
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
const evidence = JSON.parse(text(files.evidence));
const plan = text(files.plan);
const registry = text(files.registry);
const verifierTest = text(files.verifierTest);
const pkg = JSON.parse(text(files.packageJson));
const selectedPostUi = between(
  ui,
  "fn SelectedPostCard",
  "fn PublicCommentsList",
  files.ui,
);

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

for (const rustFile of rustFilesUnder(files.storefrontSrc)) {
  const source = text(rustFile);
  for (const marker of legacySummarizerMarkers) {
    assertNotContains(source, marker, `${rustFile}: removed legacy summarizer ${marker} must not return`);
  }
  for (const marker of expectedRenderContract.forbidden_storefront_markers) {
    assertNotContains(source, marker, `${rustFile}: storefront must not introduce local richtext renderer marker ${marker}`);
  }
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
assertContains(model, "RichTextView", `${files.model}: selected post DTO must consume the owner RichTextView`);
assertContains(model, "pub content: Option<RichTextView>", `${files.model}: selected post DTO must carry canonical richtext`);
assertContains(model, "pub content_plain_text: Option<String>", `${files.model}: selected post DTO must carry server-derived plain text`);
assertContains(model, "pub struct BlogCommentCreateRequest", `${files.model}: storefront must expose one typed Blog-bound comment command`);
assertContains(model, "pub content: RichTextDocument", `${files.model}: comment command must carry the canonical write document`);
assertNotContains(model, "pub body: Option<String>", `${files.model}: storefront DTO must not expose legacy body`);
assertNotContains(model, "pub body_format: String", `${files.model}: storefront DTO must not expose legacy body format`);

assertContains(ui, "use_route_query_value(comments_pagination::COMMENTS_PAGE_QUERY_KEY)", `${files.ui}: UI must read route-owned comments page state`);
assertContains(ui, "use_route_query_writer()", `${files.ui}: UI must write pagination intents through shared routing`);
assertContains(ui, "transport::fetch_blog(request, comments_page)", `${files.ui}: UI must pass the current comments page through transport`);
assertContains(selectedPostUi, "<PublicCommentsList comments=public_comments comments_page />", `${files.ui}: selected post must render paginated public comments`);
assertContains(ui, "comments_pagination::comments_page_query_intent", `${files.ui}: pagination controls must use the pure route policy`);
assertContains(selectedPostUi, "let content = post.content;", `${files.ui}: selected post must consume RichTextView from the storefront DTO`);
assertContains(selectedPostUi, "post.content_plain_text", `${files.ui}: selected post must retain server-derived plain-text fallback`);
assertContains(selectedPostUi, expectedRenderContract.fallback_sink, `${files.ui}: selected post must render the server-derived plain-text fallback`);
assertContains(ui, "RichTextHtml", `${files.ui}: UI must use the shared server-projection renderer`);
assertContains(selectedPostUi, "<CommentComposer", `${files.ui}: selected Blog post must compose the Comments-owned editor`);
const richTextHtmlSinks = [...selectedPostUi.matchAll(/<RichTextHtml\b/g)];
if (richTextHtmlSinks.length !== 1) {
  fail(`${files.ui}: SelectedPostCard must have exactly one shared RichTextHtml sink`);
}
assertContains(selectedPostUi, "view=content", `${files.ui}: RichTextHtml must receive the typed owner projection`);
assertContains(selectedPostUi, "content_locale=effective_locale.clone()", `${files.ui}: RichTextHtml must receive the owner effective locale`);
assertNotContains(selectedPostUi, "inner_html=", `${files.ui}: owner UI must not bypass the shared RichTextHtml sink`);
assertContains(cargo, "leptos-ui.workspace = true", `${files.cargo}: storefront must depend on the shared Leptos richtext view boundary`);
assertNotContains(ui, "post.body", `${files.ui}: UI must not read legacy body`);
assertNotContains(ui, "body_format", `${files.ui}: UI must not read legacy body format`);
assertNotContains(ui, "crate::api", `${files.ui}: UI must not call legacy api module`);

assertContains(transport, "pub mod graphql_adapter;", `${files.transport}: transport facade must wire GraphQL adapter`);
assertContains(transport, "pub mod native_server_adapter;", `${files.transport}: transport facade must wire native adapter`);
assertContains(transport, "comments_page: u64", `${files.transport}: transport facade must carry comments page`);
assertContains(transport, "native_server_adapter::fetch_blog(native_request, comments_page)", `${files.transport}: native path must receive comments page`);
assertContains(transport, "graphql_adapter::fetch_blog(request, comments_page)", `${files.transport}: GraphQL path must receive comments page`);
assertContains(transport, "pub async fn create_comment(", `${files.transport}: transport facade must expose the Blog-bound comment command`);
assertContains(transport, "execute_selected_transport(", `${files.transport}: comment command must use the selected transport without fallback`);
assertNotContains(transport, "crate::api", `${files.transport}: transport facade must not delegate to legacy api module`);

for (const marker of [
  "ChannelService::new",
  ".is_module_enabled(channel_id, MODULE_SLUG)",
  "normalize_channel_slug",
  "is_visible_for_public_channel",
  "request_context.channel_slug",
  "Module '{MODULE_SLUG}' is not enabled for channel",
  "CommentService::new",
  "list_public_comments_with_snapshot(",
  "SecurityContext::public_read()",
  "comments_page,",
  "COMMENTS_PAGE_SIZE,",
  "map_comment_list_item",
  "content: Some(post.content)",
  "content_plain_text: Some(post.content_plain_text)",
]) {
  assertContains(native, marker, `${files.native}: missing channel/comments/richtext native marker ${marker}`);
}
assertNotContains(native, "body: Some(post.body)", `${files.native}: native adapter must not map legacy body`);
assertNotContains(native, "body_format: post.body_format", `${files.native}: native adapter must not map legacy body format`);
assertContains(native, "#[server(prefix = \"/api/fn\", endpoint = \"blog/storefront-data\")]", `${files.native}: native adapter must own server function endpoint`);
assertContains(native, "expect_context::<HostRuntimeContext>()", `${files.native}: native adapter must use the host runtime context`);
assertContains(native, "shared_get::<TransactionalEventBus>()", `${files.native}: native adapter must receive the event bus through the host runtime context`);
assertContains(native, "runtime_ctx.db_clone()", `${files.native}: native adapter must receive DB through the host runtime context`);
assertContains(native, 'endpoint = "blog/comment-create"', `${files.native}: native adapter must expose the Blog-bound comment endpoint`);
assertContains(native, ".create_public_comment(", `${files.native}: native command must validate the public Blog target before Comments writes`);

assertContains(graphql, "GraphqlRequest", `${files.graphql}: GraphQL adapter must keep GraphQL request contract`);
assertContains(graphql, "STOREFRONT_BLOG_QUERY", `${files.graphql}: GraphQL adapter must own storefront blog query`);
assertContains(graphql, "content { document html } contentPlainText", `${files.graphql}: GraphQL storefront query must request the owner richtext projection`);
assertNotContains(graphql, " excerpt body bodyFormat ", `${files.graphql}: GraphQL storefront query must not request legacy body fields`);
assertContains(graphql, "$commentsPage: Int!", `${files.graphql}: GraphQL query must declare comments page`);
assertContains(graphql, "$commentsPerPage: Int!", `${files.graphql}: GraphQL query must declare comments page size`);
assertContains(graphql, "publicComments(locale: $locale, page: $commentsPage, perPage: $commentsPerPage)", `${files.graphql}: GraphQL storefront query must request the selected comments page`);
assertContains(graphql, "bounded_comments_request_page(comments_page)", `${files.graphql}: GraphQL page input must be bounded before serialization`);
assertContains(graphql, "comments_per_page: COMMENTS_PAGE_SIZE", `${files.graphql}: GraphQL and native page size must match`);
assertContains(graphql, "CREATE_BLOG_COMMENT_MUTATION", `${files.graphql}: GraphQL adapter must expose the Blog-bound comment mutation`);
for (const marker of [
  "pub content: RichTextView",
  "pub content_plain_text: String",
  "#[graphql(complex)]",
  "async fn public_comments(",
  "runtime.comment_service(db.clone(), event_bus.clone())",
  "list_public_comments_with_snapshot(",
  "runtime.public_comments_snapshot_store()",
  "GqlPublicCommentList",
]) {
  assertContains(graphqlTypes, marker, `${files.graphqlTypes}: missing richtext/public-comments GraphQL marker ${marker}`);
}

if (
  evidence.schema_version !== 2 ||
  evidence.owner !== "rustok-blog" ||
  evidence.boundary !== "storefront-post-richtext-view" ||
  evidence.status !== "locally_verified"
) {
  fail(`${files.evidence}: evidence identity/status drift`);
}
if (JSON.stringify(evidence.scope) !== JSON.stringify(expectedScope)) {
  fail(`${files.evidence}: evidence scope drift`);
}
if (JSON.stringify(evidence.contract) !== JSON.stringify(expectedContract)) {
  fail(`${files.evidence}: storefront richtext contract drift`);
}
if (JSON.stringify(evidence.canonical_contract) !== JSON.stringify(expectedCanonicalContract)) {
  fail(`${files.evidence}: canonical read contract drift`);
}
if (JSON.stringify(evidence.render_contract) !== JSON.stringify(expectedRenderContract)) {
  fail(`${files.evidence}: selected-post render contract drift`);
}
if (
  evidence.guardrail !== files.verifier ||
  evidence.guardrail_test !== files.verifierTest
) {
  fail(`${files.evidence}: verifier path drift`);
}
if (
  evidence.validation?.tests_run !== true ||
  evidence.validation?.verifier_run !== true ||
  evidence.validation?.cargo_run !== true ||
  evidence.validation?.format_run !== true ||
  evidence.validation?.workflow_checks_run !== false ||
  evidence.validation?.ci_run !== false
) {
  fail(`${files.evidence}: validation flags must record that execution remains maintainer-owned`);
}
if (
  !Array.isArray(evidence.remaining) ||
  evidence.remaining.length !== 1 ||
  evidence.remaining[0] !== "execute migration, live transport, and mounted browser evidence"
) {
  fail(`${files.evidence}: remaining execution contract drift`);
}

assertContains(plan, "verify-blog-storefront-boundary.mjs", `${files.plan}: local plan must mention storefront guardrail`);
assertContains(plan, "public comments", `${files.plan}: local plan must record public comment rendering parity`);
assertContains(plan, "storefront comment pagination", `${files.plan}: local plan must record route-owned comment pagination`);
assertContains(plan, "server-rendered `RichTextView` HTML", `${files.plan}: local plan must record storefront owner projection`);
assertContains(plan, "exactly one shared `RichTextHtml` sink", `${files.plan}: local plan must record the selected-post render sink`);
assertContains(registry, "verify-blog-storefront-boundary.mjs", `${files.registry}: central board must mention storefront guardrail`);
assertContains(verifierTest, "passes canonical fixture", `${files.verifierTest}: fixture tests must cover canonical pass path`);
assertContains(verifierTest, "rejects legacy api module", `${files.verifierTest}: fixture tests must reject legacy api module`);
assertContains(verifierTest, "rejects missing public comments parity", `${files.verifierTest}: fixture tests must reject missing comments parity`);
assertContains(verifierTest, "rejects missing comment pagination parity", `${files.verifierTest}: fixture tests must reject missing pagination parity`);
assertContains(verifierTest, "rejects legacy richtext transport", `${files.verifierTest}: fixture tests must reject legacy richtext transport`);
assertContains(verifierTest, "rejects nullable GraphQL richtext projections", `${files.verifierTest}: fixture tests must reject nullable GraphQL richtext projections`);
assertContains(verifierTest, "rejects removed richtext summarizer", `${files.verifierTest}: fixture tests must reject removed summarizers`);
assertContains(verifierTest, "rejects local richtext renderer", `${files.verifierTest}: fixture tests must reject local renderers`);
assertContains(verifierTest, "rejects alternate selected-post HTML sink", `${files.verifierTest}: fixture tests must reject alternate HTML sinks`);
assertContains(verifierTest, "rejects evidence false-contract drift", `${files.verifierTest}: fixture tests must reject false-contract drift`);

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
