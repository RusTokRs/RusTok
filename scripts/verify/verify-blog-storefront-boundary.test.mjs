#!/usr/bin/env node
import test from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-blog-storefront-boundary.mjs");

function writeFixtureFile(root, filePath, contents) {
  const target = path.join(root, filePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, contents);
}

function packageSource({ omitAggregate = false } = {}) {
  return JSON.stringify({
    scripts: {
      "verify:blog:storefront-boundary": "node scripts/verify/verify-blog-storefront-boundary.mjs",
      "test:verify:blog:storefront-boundary": "node scripts/verify/verify-blog-storefront-boundary.test.mjs",
      "verify:ffa:ui:migration": omitAggregate
        ? "npm run verify:blog:admin-boundary"
        : "npm run verify:blog:admin-boundary && npm run verify:blog:storefront-boundary",
      "test:verify:ffa:ui:migration": "npm run test:verify:blog:admin-boundary && npm run test:verify:blog:storefront-boundary",
    },
  });
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-blog-storefront-boundary-"));
  const pagination = !options.missingPagination;
  writeFixtureFile(root, "crates/rustok-blog/storefront/src/lib.rs", `${options.legacyApi ? "mod api;" : ""}\n${pagination ? "mod comments_pagination;" : ""}\nmod transport;\npub use ui::BlogView;\n`);
  writeFixtureFile(root, "crates/rustok-blog/storefront/src/core.rs", options.leptosCore ? "use leptos::prelude::*;" : "pub struct BlogStorefrontFetchRequest;");
  writeFixtureFile(
    root,
    "crates/rustok-blog/storefront/src/comments_pagination.rs",
    pagination
      ? `use rustok_ui_core::UiRouteQueryIntent;
const COMMENTS_PAGE_QUERY_KEY: &str = "commentsPage";
const COMMENTS_PAGE_SIZE: u64 = 20;
const MAX_COMMENTS_PAGE: u64 = 10000;
fn bounded_comments_request_page() {}
fn comments_page_from_query() {}
fn comments_total_pages() {}
fn comments_page_query_intent() { UiRouteQueryIntent::clear(COMMENTS_PAGE_QUERY_KEY); }`
      : "pub fn placeholder() {}",
  );
  writeFixtureFile(
    root,
    "crates/rustok-blog/storefront/src/model.rs",
    options.missingComments
      ? "pub struct StorefrontBlogData;"
      : "pub struct BlogCommentList; pub struct BlogPostDetail { pub public_comments: BlogCommentList }",
  );
  writeFixtureFile(
    root,
    "crates/rustok-blog/storefront/src/ui/leptos.rs",
    options.missingComments
      ? "pub fn render() { let _ = transport::fetch_blog; }"
      : pagination
        ? `use_route_query_value(comments_pagination::COMMENTS_PAGE_QUERY_KEY);
use_route_query_writer();
transport::fetch_blog(request, comments_page);
<PublicCommentsList comments=public_comments comments_page />;
comments_pagination::comments_page_query_intent;`
        : "transport::fetch_blog(request); <PublicCommentsList comments=public_comments />;",
  );
  writeFixtureFile(
    root,
    "crates/rustok-blog/storefront/src/transport/mod.rs",
    pagination
      ? "pub mod graphql_adapter; pub mod native_server_adapter; comments_page: u64; native_server_adapter::fetch_blog(native_request, comments_page); graphql_adapter::fetch_blog(request, comments_page);"
      : "pub mod graphql_adapter; pub mod native_server_adapter; native_server_adapter::fetch_blog(native_request); graphql_adapter::fetch_blog(request);",
  );
  writeFixtureFile(
    root,
    "crates/rustok-blog/storefront/src/transport/native_server_adapter.rs",
    `#[server(prefix = "/api/fn", endpoint = "blog/storefront-data")]
expect_context::<HostRuntimeContext>()
shared_get::<TransactionalEventBus>()
runtime_ctx.db_clone()
ChannelService::new
.is_module_enabled(channel_id, MODULE_SLUG)
normalize_channel_slug
is_visible_for_public_channel
request_context.channel_slug
Module '{MODULE_SLUG}' is not enabled for channel
${options.missingComments ? "" : `CommentService::new
.list_for_post_with_locale_fallback(
SecurityContext::public_read()
${pagination ? "page: comments_page.max(1)\nper_page: COMMENTS_PAGE_SIZE\n" : ""}map_comment_list_item
`}`,
  );
  writeFixtureFile(
    root,
    "crates/rustok-blog/storefront/src/transport/graphql_adapter.rs",
    `use rustok_graphql::GraphqlRequest;
const STOREFRONT_BLOG_QUERY: &str = "${options.missingComments ? "" : pagination ? "$commentsPage: Int! $commentsPerPage: Int! publicComments(locale: $locale, page: $commentsPage, perPage: $commentsPerPage)" : "publicComments(locale: $locale"}";
${pagination ? "bounded_comments_request_page(comments_page); comments_per_page: COMMENTS_PAGE_SIZE;" : ""}`,
  );
  writeFixtureFile(
    root,
    "crates/rustok-blog/src/graphql/types.rs",
    options.missingComments
      ? "pub struct GqlPost;"
      : "#[graphql(complex)] pub struct GqlPost; async fn public_comments() { CommentService::new; SecurityContext::public_read(); GqlPublicCommentList; }",
  );
  if (options.legacyApi) writeFixtureFile(root, "crates/rustok-blog/storefront/src/api.rs", "legacy api");
  writeFixtureFile(root, "crates/rustok-blog/storefront/Cargo.toml", "[package]\nname = \"rustok-blog-storefront-fixture\"\nversion = \"0.1.0\"\n");
  writeFixtureFile(root, "crates/rustok-blog/docs/implementation-plan.md", `verify-blog-storefront-boundary.mjs public comments ${pagination ? "storefront comment pagination" : ""}`);
  writeFixtureFile(root, "docs/modules/registry.md", "verify-blog-storefront-boundary.mjs");
  writeFixtureFile(root, "scripts/verify/verify-blog-storefront-boundary.test.mjs", "passes canonical fixture\nrejects legacy api module\nrejects missing public comments parity\nrejects missing comment pagination parity\n");
  writeFixtureFile(root, "package.json", packageSource(options));
  return root;
}

function run(root) {
  return spawnSync(process.execPath, [scriptPath], { cwd: root, encoding: "utf8" });
}

test("blog storefront boundary verifier passes canonical fixture", () => {
  const result = run(fixture());
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /blog storefront boundary verification passed/);
});

test("blog storefront boundary verifier rejects legacy api module", () => {
  const result = run(fixture({ legacyApi: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /legacy api\.rs/);
});

test("blog storefront boundary verifier rejects Leptos-specific core", () => {
  const result = run(fixture({ leptosCore: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /core must remain framework\/server-function free/);
});

test("blog storefront boundary verifier rejects missing package aggregate wiring", () => {
  const result = run(fixture({ omitAggregate: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /aggregate FFA verifier must include blog storefront verifier/);
});

test("blog storefront boundary verifier rejects missing public comments parity", () => {
  const result = run(fixture({ missingComments: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /public comment list|public comments|approved public comments/);
});

test("blog storefront boundary verifier rejects missing comment pagination parity", () => {
  const result = run(fixture({ missingPagination: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /pagination|comments page|commentsPage/);
});
