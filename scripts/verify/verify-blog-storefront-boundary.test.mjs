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
  writeFixtureFile(root, "crates/rustok-blog/storefront/src/lib.rs", `${options.legacyApi ? "mod api;" : ""}\nmod transport;\npub use ui::BlogView;\n`);
  writeFixtureFile(root, "crates/rustok-blog/storefront/src/core.rs", options.leptosCore ? "use leptos::prelude::*;" : "pub struct BlogStorefrontFetchRequest;");
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
      : "pub fn render() { let _ = transport::fetch_blog; <PublicCommentsList comments=public_comments />; }",
  );
  writeFixtureFile(root, "crates/rustok-blog/storefront/src/transport/mod.rs", "pub mod graphql_adapter;\npub mod native_server_adapter;\npub async fn fetch_blog() { native_server_adapter::fetch_blog().await; graphql_adapter::fetch_blog().await; }");
  writeFixtureFile(
    root,
    "crates/rustok-blog/storefront/src/transport/native_server_adapter.rs",
    `#[server(prefix = "/api/fn", endpoint = "blog/storefront-data")]
async fn endpoint() {}
expect_context::<HostRuntimeContext>()
shared_get::<TransactionalEventBus>()
runtime_ctx.db_clone()
ChannelService::new
.is_module_enabled(channel_id, MODULE_SLUG)
normalize_channel_slug
is_visible_for_public_channel
request_context.channel_slug
Module '{MODULE_SLUG}' is not enabled for channel
${options.missingComments ? "" : "CommentService::new\n.list_for_post_with_locale_fallback(\nSecurityContext::public_read()\nmap_comment_list_item\n"}`,
  );
  writeFixtureFile(
    root,
    "crates/rustok-blog/storefront/src/transport/graphql_adapter.rs",
    `use rustok_graphql::GraphqlRequest;
const STOREFRONT_BLOG_QUERY: &str = "${options.missingComments ? "" : "publicComments(locale: $locale"}";`,
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
  writeFixtureFile(root, "crates/rustok-blog/docs/implementation-plan.md", "verify-blog-storefront-boundary.mjs public comments");
  writeFixtureFile(root, "docs/modules/registry.md", "verify-blog-storefront-boundary.mjs");
  writeFixtureFile(root, "scripts/verify/verify-blog-storefront-boundary.test.mjs", "passes canonical fixture\nrejects legacy api module\nrejects missing public comments parity\n");
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
