#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-forum-admin-boundary.mjs");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function libSource({ omitTransport = false } = {}) {
  return `
mod core;
${omitTransport ? "" : "mod transport;"}
mod ui;

pub use ui::leptos::ForumAdmin;
`;
}

function coreSource({ includeLeptos = false, omitDeleteOutcome = false } = {}) {
  const helpers = [
    "CategoryFormSnapshot", "TopicFormSnapshot", "ForumAdminRouteQueryIntent", "ForumAdminDeleteOutcome",
    "forum_admin_delete_outcome", "forum_admin_busy_key", "ForumAdminBusySurface", "ForumAdminFormErrorLabels",
    "ForumAdminCategorySelectOption", "category_select_options", "forum_admin_topic_tag_count_label",
    "forum_admin_editing_thread_label", "forum_admin_position_value", "forum_admin_sidebar_category_class",
    "forum_admin_status_badge_class", "forum_admin_tag_chips", "forum_admin_title_envelope_view_model",
    "forum_admin_placeholder_policy", "forum_admin_seo_copy_labels", "forum_admin_form_error_message",
    "forum_admin_transport_error_message", "selected_category_filter_label", "forum_admin_collection_state",
    "category_card_view_model", "topic_card_view_model", "ForumAdminModeratorNotesLabels",
    "forum_admin_moderator_notes_copy_labels", "ForumAdminSidebarLabels", "forum_admin_sidebar_copy_labels",
    "ForumAdminMetricSurface", "forum_admin_metric_accent_class", "ForumAdminActionButtonKind",
    "forum_admin_action_button_class", "ForumAdminCategoryMatrixLabels", "forum_admin_category_matrix_labels",
    "ForumAdminCategoryFormLabels", "forum_admin_category_form_labels", "ForumAdminTopicStreamLabels",
    "forum_admin_topic_stream_labels", "ForumAdminTopicFormLabels", "forum_admin_topic_form_labels",
    "ForumAdminReplyPreviewLabels", "forum_admin_reply_preview_labels",
  ].filter((name) => !(omitDeleteOutcome && name === "forum_admin_delete_outcome"));
  return `
${includeLeptos ? "use leptos::prelude::*;" : ""}
${helpers.map((name) => name.startsWith("Forum") || name.endsWith("Snapshot") ? `pub struct ${name};` : `pub fn ${name}() {}`).join("\n")}
`;
}

function modelSource({ modelUsesLeptos = false } = {}) {
  return `
${modelUsesLeptos ? "use leptos::prelude::*;" : ""}
pub struct CategoryTreeResponse;
pub enum CategoryDropPlacement { Before, Inside, RootEnd }
pub struct CategoryMoveRequest;
pub fn category_drop_move_request() {}
pub fn into_flat_items() {}
`;
}

function uiSource({
  rawApi = false,
  rawService = false,
  rawBusy = false,
  omitCoreImport = false,
  flatCategoryRead = false,
} = {}) {
  return `
${omitCoreImport ? "" : "use crate::core::{forum_admin_category_matrix_labels, forum_admin_category_form_labels, forum_admin_topic_stream_labels, forum_admin_topic_form_labels, forum_admin_reply_preview_labels, forum_admin_busy_key, forum_admin_form_error_message, forum_admin_transport_error_message, category_select_options, forum_admin_topic_tag_count_label, forum_admin_editing_thread_label, forum_admin_position_value, forum_admin_sidebar_category_class, forum_admin_status_badge_class, forum_admin_tag_chips, forum_admin_title_envelope_view_model, forum_admin_placeholder_policy, forum_admin_seo_copy_labels, forum_admin_delete_outcome, CategoryFormSnapshot, TopicFormSnapshot, forum_admin_moderator_notes_copy_labels, forum_admin_sidebar_copy_labels, forum_admin_metric_accent_class, forum_admin_action_button_class};"}
use crate::transport;
use crate::ui::category_dnd::CategoryDndGrid;

pub mod leptos {
  pub fn ForumAdmin() {
    let _ = super::transport::fetch_category_tree;
    let _ = CategoryDndGrid;
    ${flatCategoryRead ? "let _ = super::transport::fetch_categories(token_value, tenant_value, locale);" : ""}
    ${rawApi ? "let _ = api::fetch_categories;" : ""}
    ${rawService ? "let _ = ForumService;" : ""}
    ${rawBusy ? "let _ = \"category:save\";" : ""}
  }
}
`;
}

function categoryDndSource({ dndUpdateBypass = false } = {}) {
  return `
use crate::model::{category_drop_move_request, CategoryDropPlacement};
use crate::transport;

pub fn CategoryDndGrid() {
  let _ = CategoryDropPlacement::Before;
  let _ = CategoryDropPlacement::Inside;
  let _ = CategoryDropPlacement::RootEnd;
  let _ = "on:dragstart";
  let _ = "on:drop";
  let _ = category_drop_move_request;
  let _ = transport::move_category;
  ${dndUpdateBypass ? "let _ = transport::update_category(category_id);" : ""}
}
`;
}

function transportSource() {
  return `
mod category_tree_graphql_adapter;
mod category_tree_rest_adapter;
mod graphql_adapter;
mod rest_adapter;
pub async fn fetch_category_tree() {
  match category_tree_graphql_adapter::fetch_category_tree().await {
    Ok(tree) => Ok(tree),
    Err(_) => category_tree_rest_adapter::fetch_category_tree().await,
  }
}
pub async fn fetch_categories() {
  match graphql_adapter::fetch_categories().await {
    Ok(categories) => Ok(categories),
    Err(_) => rest_adapter::fetch_categories().await,
  }
}
pub async fn fetch_category() {}
pub async fn create_category() {}
pub async fn update_category() {}
pub async fn move_category() {
  match graphql_adapter::move_category().await {
    Ok(()) => Ok(()),
    Err(_) => rest_adapter::move_category().await,
  }
}
pub async fn reorder_category_siblings() {
  match graphql_adapter::reorder_category_siblings().await {
    Ok(()) => Ok(()),
    Err(_) => rest_adapter::reorder_category_siblings().await,
  }
}
pub async fn delete_category() {}
pub async fn fetch_topics() {}
pub async fn fetch_topic() {}
pub async fn create_topic() {}
pub async fn update_topic() {}
pub async fn delete_topic() {}
pub async fn fetch_replies() {}
`;
}

function graphqlAdapterSource() {
  return `
pub async fn fetch_categories() {}
pub async fn move_category() {}
pub async fn reorder_category_siblings() {}
const MOVE: &str = "moveForumCategory";
const REORDER: &str = "reorderForumCategorySiblings";
`;
}

function restAdapterSource({ rawPlacementBypass = false } = {}) {
  return `
use reqwest;
pub async fn fetch_categories() {}
pub async fn move_category() { let _ = "/categories/{id}/move"; }
pub async fn reorder_category_siblings() { let _ = "/categories/reorder"; }
${rawPlacementBypass ? "fn bypass() { let _ = position: Some(draft.position); }" : ""}
`;
}

function categoryTreeGraphqlAdapterSource() {
  return `
const MAX_CATEGORY_TREE_DEPTH: u8 = 16;
const QUERY: &str = "forumCategoryTree archived_at: archivedAt";
pub async fn fetch_category_tree() {}
`;
}

function categoryTreeRestAdapterSource() {
  return `
pub async fn fetch_category_tree() { let _ = "/categories/tree"; }
`;
}

function packageJsonSource({ omitPackageScript = false, omitAggregateForumTest = false } = {}) {
  const scripts = {
    ...(omitPackageScript ? {} : { "test:verify:forum:admin-boundary": "node scripts/verify/verify-forum-admin-boundary.test.mjs" }),
    "test:verify:ffa:ui:migration": omitAggregateForumTest
      ? "node scripts/verify/verify-ffa-ui-migration-contract.test.mjs"
      : "node scripts/verify/verify-ffa-ui-migration-contract.test.mjs && npm run test:verify:forum:admin-boundary",
  };
  return JSON.stringify({ scripts }, null, 2);
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-forum-boundary-"));
  writeFixtureFile(root, "crates/rustok-forum/admin/src/lib.rs", libSource(options));
  writeFixtureFile(root, "crates/rustok-forum/admin/src/core.rs", coreSource(options));
  writeFixtureFile(root, "crates/rustok-forum/admin/src/model.rs", modelSource(options));
  writeFixtureFile(root, "crates/rustok-forum/admin/src/ui/leptos.rs", uiSource(options));
  writeFixtureFile(root, "crates/rustok-forum/admin/src/ui/category_dnd.rs", categoryDndSource(options));
  writeFixtureFile(root, "crates/rustok-forum/admin/src/transport.rs", transportSource());
  writeFixtureFile(root, "crates/rustok-forum/admin/src/transport/graphql_adapter.rs", graphqlAdapterSource());
  writeFixtureFile(root, "crates/rustok-forum/admin/src/transport/rest_adapter.rs", restAdapterSource(options));
  writeFixtureFile(root, "crates/rustok-forum/admin/src/transport/category_tree_graphql_adapter.rs", categoryTreeGraphqlAdapterSource());
  writeFixtureFile(root, "crates/rustok-forum/admin/src/transport/category_tree_rest_adapter.rs", categoryTreeRestAdapterSource());
  if (options.legacyApi) writeFixtureFile(root, "crates/rustok-forum/admin/src/api.rs", "use reqwest;\n");
  writeFixtureFile(root, "crates/rustok-forum/docs/implementation-plan.md", "verify-forum-admin-boundary.mjs interactive admin drag-and-drop");
  writeFixtureFile(root, "docs/modules/registry.md", "verify-forum-admin-boundary.mjs forum-wave1-rollout-evidence.json");
  writeFixtureFile(root, "package.json", packageJsonSource(options));
  writeFixtureFile(
    root,
    "scripts/verify/verify-forum-admin-boundary.test.mjs",
    "passes canonical fixture\nrejects Leptos-specific core\nrejects raw api calls from UI\nrejects legacy admin api module\nrejects raw busy-key strings from UI\nrejects flat category hierarchy reads\nrejects DnD generic update bypass\nrejects missing package fixture script\n",
  );
  return root;
}

function runVerifier(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

function withTempFixture(options, assertion) {
  const root = withFixture(options);
  try {
    assertion(runVerifier(root));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("forum admin boundary verifier passes canonical fixture", () => {
  withTempFixture({}, (result) => {
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /forum admin boundary verification passed/);
  });
});

test("forum admin boundary verifier rejects Leptos-specific core", () => {
  withTempFixture({ includeLeptos: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /core must stay Leptos\/server-function free/);
  });
});

test("forum admin boundary verifier rejects raw api calls from UI", () => {
  withTempFixture({ rawApi: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /UI adapter must not call raw transport or services/);
  });
});

test("forum admin boundary verifier rejects legacy admin api module", () => {
  withTempFixture({ legacyApi: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /legacy api\.rs/);
  });
});

test("forum admin boundary verifier rejects raw busy-key strings from UI", () => {
  withTempFixture({ rawBusy: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /busy-key strings must stay in core helper/);
  });
});

test("forum admin boundary verifier rejects generic category position bypass", () => {
  withTempFixture({ rawPlacementBypass: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not bypass placement owner commands/);
  });
});

test("forum admin boundary verifier rejects flat category hierarchy reads", () => {
  withTempFixture({ flatCategoryRead: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not be reconstructed from the flat compatibility list/);
  });
});

test("forum admin boundary verifier rejects DnD generic update bypass", () => {
  withTempFixture({ dndUpdateBypass: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /DnD must not bypass the transport owner move command/);
  });
});

test("forum admin boundary verifier rejects missing package fixture script", () => {
  withTempFixture({ omitPackageScript: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /package scripts must expose forum boundary fixture tests/);
  });
});

test("forum admin boundary verifier rejects aggregate test script without forum fixtures", () => {
  withTempFixture({ omitAggregateForumTest: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /aggregate FFA fixture tests must include forum boundary fixtures/);
  });
});
