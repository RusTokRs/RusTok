#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-pages-ui-boundary.mjs");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function adminLibSource({ publicTransportPassthrough = false } = {}) {
  return `
mod builder;
mod composition;
mod core;
mod transport;
pub use composition::PagesAdmin;
${publicTransportPassthrough ? "pub async fn fetch_pages() {}" : ""}
`;
}

function adminBuilderSource({ legacyFrameSync = false } = {}) {
  return `
pub struct PagesBuilderFacade;
impl PageBuilderAdminFacade for PagesBuilderFacade {}
fn current() {
  let _ = PageBuilderCapabilityRequest::Publish;
  let _ = transport::fetch_page;
  let _ = transport::update_page;
  let _ = REVISION_CONFLICT;
  let _ = canonicalize_builder_project;
  let _ = "pages[].component";
  ${legacyFrameSync ? "let _ = copy_frame_component; let _ = synchronize_frame_component;" : ""}
}
`;
}

function adminCompositionSource({ parallelUi = false, rawAdapter = false } = {}) {
  return `
pub fn PagesAdmin() {
  let _ = CreatePageCard;
  let _ = PagesNavigator;
  let _ = PageWorkspace;
  let _ = transport::fetch_pages;
  let _ = transport::fetch_page;
  let _ = transport::create_page;
  let _ = transport::publish_page;
  let _ = transport::unpublish_page;
  let _ = transport::delete_page;
  let _ = PageBuilderAdminHostContext;
  let _ = PageBuilderAdmin;
  ${parallelUi ? "let _ = crate::ui::leptos::PagesAdmin; let _ = \"<textarea\";" : ""}
  ${rawAdapter ? "let _ = graphql_adapter::fetch_pages;" : ""}
}
`;
}

function adminCoreSource({ legacyBlock = false } = {}) {
  return `
pub struct PageDraftFormInput;
pub fn build_create_page_draft() {}
pub fn edit_form_seed_from_page() {}
pub fn default_project_data() {}
pub fn parse_project_data() {}
pub fn status_badge_class() {}
${legacyBlock ? "pub struct PageBlock; pub fn legacy_block_snapshot_label() {}" : ""}
`;
}

function adminModelSource({ legacyBlock = false } = {}) {
  return `
pub struct PageList;
pub struct PageDetail;
pub struct PageMutationResult;
${legacyBlock ? "pub struct PageBlock; pub blocks: Vec<PageBlock>," : ""}
`;
}

function adminTransportSource({ includeServerEndpoint = false } = {}) {
  return `
mod graphql_adapter;
pub async fn fetch_pages() {}
pub async fn fetch_page() {}
pub async fn create_page() {}
pub async fn update_page() {}
pub async fn publish_page() {}
pub async fn unpublish_page() {}
pub async fn delete_page() {}
${includeServerEndpoint ? '#[server(prefix = "/api/fn", endpoint = "bad")] async fn bad() {}' : ""}
`;
}

function adminGraphqlSource({ blocks = false } = {}) {
  return `
use rustok_graphql::GraphqlRequest;
const PAGE_QUERY: &str = "page { id body { contentJson } ${blocks ? "blocks { id }" : ""} }";
${blocks ? "struct CreatePageInput { blocks: Option<Vec<()>> }" : ""}
`;
}

function storefrontLibSource() {
  return `
mod core;
mod transport;
mod ui;
pub use ui::leptos::PagesView;
`;
}

function storefrontCoreSource() {
  return `
pub fn selected_page_title() {}
pub fn selected_page_empty_state() {}
pub fn load_error_message() {}
`;
}

function storefrontUiSource() {
  return `
use crate::core;
use crate::transport;
pub fn PagesView() {
  let _ = core::selected_page_title;
  let _ = core::selected_page_empty_state;
  let _ = core::load_error_message;
  let _ = transport::fetch_pages;
}
`;
}

function storefrontTransportSource() {
  return `
mod graphql_adapter;
mod native_server_adapter;
pub async fn fetch_pages() {}
`;
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-pages-boundary-"));
  writeFixtureFile(root, "crates/rustok-pages/admin/Cargo.toml", `
[dependencies]
rustok-page-builder = { path = "../../rustok-page-builder" }
rustok-page-builder-admin = { path = "../../rustok-page-builder/admin" }
`);
  writeFixtureFile(root, "crates/rustok-pages/admin/src/lib.rs", adminLibSource(options));
  writeFixtureFile(root, "crates/rustok-pages/admin/src/builder.rs", adminBuilderSource(options));
  writeFixtureFile(root, "crates/rustok-pages/admin/src/composition.rs", adminCompositionSource(options));
  writeFixtureFile(root, "crates/rustok-pages/admin/src/core.rs", adminCoreSource(options));
  writeFixtureFile(root, "crates/rustok-pages/admin/src/model.rs", adminModelSource(options));
  writeFixtureFile(root, "crates/rustok-pages/admin/src/transport/mod.rs", adminTransportSource(options));
  writeFixtureFile(root, "crates/rustok-pages/admin/src/transport/graphql_adapter.rs", adminGraphqlSource(options));

  if (options.legacyAdminUi) {
    writeFixtureFile(root, "crates/rustok-pages/admin/src/ui/leptos.rs", "pub fn PagesAdmin() {}");
  }

  writeFixtureFile(root, "crates/rustok-pages/storefront/src/lib.rs", storefrontLibSource());
  writeFixtureFile(root, "crates/rustok-pages/storefront/src/core.rs", storefrontCoreSource());
  writeFixtureFile(root, "crates/rustok-pages/storefront/src/ui/leptos.rs", storefrontUiSource());
  writeFixtureFile(root, "crates/rustok-pages/storefront/src/transport/mod.rs", storefrontTransportSource());
  writeFixtureFile(root, "crates/rustok-pages/storefront/src/transport/graphql_adapter.rs", "use rustok_graphql::GraphqlRequest;");
  writeFixtureFile(root, "crates/rustok-pages/storefront/src/transport/native_server_adapter.rs", `
#[server(prefix = "/api/fn", endpoint = "pages")]
async fn pages() { expect_context::<HostRuntimeContext>(); }
`);
  writeFixtureFile(root, "crates/rustok-pages/docs/implementation-plan.md", "verify-pages-ui-boundary.mjs\nno legacy\n");
  writeFixtureFile(root, "docs/modules/registry.md", "verify-pages-ui-boundary.mjs\n");
  return root;
}

function runVerifier(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

function expectFailure(options, pattern) {
  const root = withFixture(options);
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected Pages boundary fixture to fail");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("pages UI boundary verifier passes builder-only fixture", () => {
  const root = withFixture();
  try {
    const result = runVerifier(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /pages UI boundary verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("rejects deleted admin UI returning", () => {
  expectFailure({ legacyAdminUi: true }, /obsolete Pages surface must stay deleted/);
});

test("rejects frame compatibility helpers", () => {
  expectFailure({ legacyFrameSync: true }, /legacy frame compatibility marker/);
});

test("rejects legacy block model", () => {
  expectFailure({ legacyBlock: true }, /obsolete (UI helper|block model marker)/);
});

test("rejects parallel JSON UI composition", () => {
  expectFailure({ parallelUi: true }, /obsolete parallel UI marker/);
});

test("rejects raw adapter selection", () => {
  expectFailure({ rawAdapter: true }, /must not select a raw transport adapter/);
});

test("rejects blocks in current GraphQL transport", () => {
  expectFailure({ blocks: true }, /obsolete block transport marker/);
});

test("rejects public crate-root transport passthroughs", () => {
  expectFailure({ publicTransportPassthrough: true }, /crate root must not expose transport passthroughs/);
});

test("rejects server functions in transport facade", () => {
  expectFailure({ includeServerEndpoint: true }, /server functions must not live in transport facade/);
});
