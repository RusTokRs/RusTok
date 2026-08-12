#!/usr/bin/env node
import test from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
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

function evidenceSource({ evidenceFalseContractDrift = false } = {}) {
  return JSON.stringify({
    schema_version: 2,
    owner: "rustok-blog",
    boundary: "storefront-post-richtext-view",
    status: "locally_verified",
    scope: [
      "crates/rustok-blog/storefront/src/core.rs",
      "crates/rustok-blog/storefront/src/model.rs",
      "crates/rustok-blog/storefront/src/transport/graphql_adapter.rs",
      "crates/rustok-blog/storefront/src/transport/native_server_adapter.rs",
      "crates/rustok-blog/storefront/src/ui/leptos.rs",
    ],
    contract: {
      graphql_owner_view: true,
      native_owner_view: true,
      server_html_render: true,
      plain_text_fallback: true,
      legacy_body_transport: false,
      legacy_body_format_transport: false,
      local_format_renderer: evidenceFalseContractDrift,
      legacy_summarizer_removed: true,
    },
    canonical_contract: {
      read: "rustok_api::RichTextView",
      html: "server-derived",
      plain_text: "server-derived",
    },
    render_contract: {
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
    },
    guardrail: "scripts/verify/verify-blog-storefront-boundary.mjs",
    guardrail_test: "scripts/verify/verify-blog-storefront-boundary.test.mjs",
    validation: {
      tests_run: true,
      verifier_run: true,
      cargo_run: true,
      format_run: true,
      workflow_checks_run: false,
      ci_run: false,
    },
    remaining: [
      "execute migration, live transport, and mounted browser evidence",
    ],
  });
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-blog-storefront-boundary-"));
  const pagination = !options.missingPagination;
  const canonicalRichtext = !options.legacyRichtext;

  writeFixtureFile(
    root,
    "crates/rustok-blog/storefront/src/lib.rs",
    `${options.legacyApi ? "mod api;" : ""}
${pagination ? "mod comments_pagination;" : ""}
mod transport;
pub use ui::BlogView;
`,
  );
  writeFixtureFile(
    root,
    "crates/rustok-blog/storefront/src/core.rs",
    options.leptosCore
      ? "use leptos::prelude::*;"
      : "pub struct BlogStorefrontFetchRequest;",
  );
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
      : canonicalRichtext
        ? "use rustok_api::{RichTextDocument, RichTextView}; pub struct BlogCommentList; pub struct BlogCommentCreateRequest { pub content: RichTextDocument } pub struct BlogPostDetail { pub content: Option<RichTextView>, pub content_plain_text: Option<String>, pub public_comments: BlogCommentList }"
        : "pub struct BlogCommentList; pub struct BlogPostDetail { pub body: Option<String>, pub body_format: String, pub public_comments: BlogCommentList }",
  );

  const selectedRichtext = canonicalRichtext
    ? `let effective_locale = post.effective_locale;
let content = post.content;
post.content_plain_text;
selected_post_content.body;
${options.alternateHtmlSink ? "inner_html=rendered_html;" : "<RichTextHtml view=content content_locale=effective_locale.clone() />;"}
${options.localRenderer ? "let _: RichTextDocument; content.document; render_richtext(content.document);" : ""}`
    : "post.body; body_format; summarized_body_or_fallback;";
  const selectedComments = options.missingComments
    ? ""
    : pagination
      ? "<PublicCommentsList comments=public_comments comments_page />;"
      : "<PublicCommentsList comments=public_comments />;";
  writeFixtureFile(
    root,
    "crates/rustok-blog/storefront/src/ui/leptos.rs",
    `use leptos_ui::RichTextHtml;
${pagination ? `use_route_query_value(comments_pagination::COMMENTS_PAGE_QUERY_KEY);
use_route_query_writer();
transport::fetch_blog(request, comments_page);
comments_pagination::comments_page_query_intent;` : "transport::fetch_blog(request);"}
fn SelectedPostCard() {
${selectedRichtext}
<CommentComposer />;
${selectedComments}
}
fn PublicCommentsList() {}
`,
  );

  writeFixtureFile(
    root,
    "crates/rustok-blog/storefront/src/transport/mod.rs",
    pagination
      ? `pub mod graphql_adapter;
pub mod native_server_adapter;
comments_page: u64;
native_server_adapter::fetch_blog(native_request, comments_page);
graphql_adapter::fetch_blog(request, comments_page);
pub async fn create_comment() {}
execute_selected_transport(
${options.legacySummarizerConsumer ? "summarize_content(content, format, template);" : ""}`
      : `pub mod graphql_adapter;
pub mod native_server_adapter;
native_server_adapter::fetch_blog(native_request);
graphql_adapter::fetch_blog(request);`,
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
list_public_comments_with_snapshot(
SecurityContext::public_read()
${pagination ? "comments_page,\nCOMMENTS_PAGE_SIZE,\n" : ""}map_comment_list_item
`}
${canonicalRichtext ? "content: Some(post.content)\ncontent_plain_text: Some(post.content_plain_text)" : "body: Some(post.body)\nbody_format: post.body_format"}
#[server(prefix = "/api/fn", endpoint = "blog/comment-create")]
.create_public_comment(`,
  );
  writeFixtureFile(
    root,
    "crates/rustok-blog/storefront/src/transport/graphql_adapter.rs",
    `use rustok_graphql::GraphqlRequest;
const STOREFRONT_BLOG_QUERY: &str = "${canonicalRichtext ? "content { document html } contentPlainText" : " excerpt body bodyFormat "} ${options.missingComments ? "" : pagination ? "$commentsPage: Int! $commentsPerPage: Int! publicComments(locale: $locale, page: $commentsPage, perPage: $commentsPerPage)" : "publicComments(locale: $locale"}";
const CREATE_BLOG_COMMENT_MUTATION: &str = "mutation";
${pagination ? "bounded_comments_request_page(comments_page); comments_per_page: COMMENTS_PAGE_SIZE;" : ""}`,
  );
  writeFixtureFile(
    root,
    "crates/rustok-blog/src/graphql/types.rs",
    options.missingComments
      ? "pub struct GqlPost;"
      : `${options.nullableGraphqlRichtext ? "pub content: Option<RichTextView>; pub content_plain_text: Option<String>;" : "pub content: RichTextView; pub content_plain_text: String;"} #[graphql(complex)] pub struct GqlPost; async fn public_comments() { runtime.comment_service(db.clone(), event_bus.clone()); list_public_comments_with_snapshot(runtime.public_comments_snapshot_store()); GqlPublicCommentList; }`,
  );
  if (options.legacyApi) {
    writeFixtureFile(root, "crates/rustok-blog/storefront/src/api.rs", "legacy api");
  }
  writeFixtureFile(
    root,
    "crates/rustok-blog/storefront/Cargo.toml",
    "[package]\nname = \"rustok-blog-storefront-fixture\"\nversion = \"0.1.0\"\n[dependencies]\nleptos-ui.workspace = true\n",
  );
  writeFixtureFile(
    root,
    "crates/rustok-blog/contracts/evidence/blog-storefront-richtext-view.json",
    evidenceSource(options),
  );
  writeFixtureFile(
    root,
    "crates/rustok-blog/docs/implementation-plan.md",
    `verify-blog-storefront-boundary.mjs public comments ${
      pagination ? "storefront comment pagination" : ""
    } server-rendered \`RichTextView\` HTML exactly one shared \`RichTextHtml\` sink`,
  );
  writeFixtureFile(
    root,
    "docs/modules/registry.md",
    "verify-blog-storefront-boundary.mjs",
  );
  writeFixtureFile(
    root,
    "scripts/verify/verify-blog-storefront-boundary.test.mjs",
    `passes canonical fixture
rejects legacy api module
rejects missing public comments parity
rejects missing comment pagination parity
rejects legacy richtext transport
rejects nullable GraphQL richtext projections
rejects removed richtext summarizer
rejects local richtext renderer
rejects alternate selected-post HTML sink
rejects evidence false-contract drift
`,
  );
  writeFixtureFile(root, "package.json", packageSource(options));
  return root;
}

function runFixture(options = {}) {
  const root = fixture(options);
  try {
    return spawnSync(process.execPath, [scriptPath], {
      cwd: root,
      encoding: "utf8",
    });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("blog storefront boundary verifier passes canonical fixture", () => {
  const result = runFixture();
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /blog storefront boundary verification passed/);
});

test("blog storefront boundary verifier rejects legacy api module", () => {
  const result = runFixture({ legacyApi: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /legacy api\.rs/);
});

test("blog storefront boundary verifier rejects Leptos-specific core", () => {
  const result = runFixture({ leptosCore: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /core must remain framework\/server-function free/);
});

test("blog storefront boundary verifier rejects missing package aggregate wiring", () => {
  const result = runFixture({ omitAggregate: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /aggregate FFA verifier must include blog storefront verifier/);
});

test("blog storefront boundary verifier rejects missing public comments parity", () => {
  const result = runFixture({ missingComments: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /public comment list|public comments|selected posts must carry public comments/);
});

test("blog storefront boundary verifier rejects missing comment pagination parity", () => {
  const result = runFixture({ missingPagination: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /pagination|comments page|commentsPage/);
});

test("blog storefront boundary verifier rejects legacy richtext transport", () => {
  const result = runFixture({ legacyRichtext: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /legacy body|owner RichTextView|canonical richtext|owner HTML sink|removed legacy summarizer/);
});

test("blog storefront boundary verifier rejects nullable GraphQL richtext projections", () => {
  const result = runFixture({ nullableGraphqlRichtext: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /pub content: RichTextView/);
});

test("blog storefront boundary verifier rejects removed richtext summarizer", () => {
  const result = runFixture({ legacySummarizerConsumer: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /removed legacy summarizer/);
});

test("blog storefront boundary verifier rejects local richtext renderer", () => {
  const result = runFixture({ localRenderer: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /local richtext renderer marker/);
});

test("blog storefront boundary verifier rejects alternate selected-post HTML sink", () => {
  const result = runFixture({ alternateHtmlSink: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /exactly one shared RichTextHtml sink/);
});

test("blog storefront boundary verifier rejects evidence false-contract drift", () => {
  const result = runFixture({ evidenceFalseContractDrift: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /storefront richtext contract drift/);
});
