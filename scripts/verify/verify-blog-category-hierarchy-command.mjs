#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const root = path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT || process.cwd());
const failures = [];

function read(relative) {
  const target = path.join(root, relative);
  if (!fs.existsSync(target)) {
    failures.push(`${relative}: expected file is missing`);
    return "";
  }
  return fs.readFileSync(target, "utf8");
}

function requireMarkers(relative, markers) {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${relative}: missing ${marker}`);
  }
}

requireMarkers("crates/rustok-blog/src/dto/category.rs", [
  "Compatibility field retained for decoding only",
  "use `MoveCategoryInput` instead",
]);

requireMarkers("crates/rustok-blog/src/dto/category_command.rs", [
  "pub struct MoveCategoryInput",
  "pub parent_id: Option<Uuid>",
  "pub position: u32",
  "pub struct CategoryPlacementResponse",
  "pub depth: i32",
  "MAX_BLOG_CATEGORY_TREE_NODES",
]);

requireMarkers("crates/rustok-blog/src/services/category.rs", [
  "lock_category_tree_for_create_in_tx(&txn, tenant_id).await?",
  "ensure_category_tree_capacity_in_tx(&txn, tenant_id).await?",
  "canonicalize_siblings_for_insert_in_tx",
  "Category position cannot be negative",
  "exceeds sibling count",
  "Blog category tree cannot exceed",
  "if input.position.is_some()",
  "Category position is structural; use the category move command",
  'format!("blog-category-tree:{tenant_id}")',
]);
const localizedUpdate = read("crates/rustok-blog/src/services/category.rs");
const updateStart = localizedUpdate.indexOf("pub async fn update(");
const updateEnd = localizedUpdate.indexOf("pub async fn delete(", updateStart);
const updateBody =
  updateStart >= 0 && updateEnd > updateStart
    ? localizedUpdate.slice(updateStart, updateEnd)
    : "";
if (updateBody.includes("Column::Position")) {
  failures.push(
    "crates/rustok-blog/src/services/category.rs: localized update must not write hierarchy position",
  );
}

requireMarkers("crates/rustok-blog/src/services/category_command.rs", [
  "pub struct CategoryCommandService",
  "Resource::BlogCategories, Action::Manage",
  "lock_category_tree_in_tx(&txn, tenant_id).await?",
  "pg_advisory_xact_lock",
  'format!("blog-category-tree:{tenant_id}")',
  "validate_and_compute_depths(&parent_by_id)?",
  "parent_by_id.insert(category_id, input.parent_id)",
  "persist_sibling_order",
  "persist_descendant_depth_changes",
  "Blog category hierarchy cycle",
  "MAX_BLOG_CATEGORY_TREE_NODES",
  "DomainEvent::ReindexRequested",
  'target_type: "blog".to_string()',
  "txn.commit().await?",
]);

requireMarkers("crates/rustok-blog/src/entities/blog_category.rs", [
  "lock_category_tree_for_insert(db, tenant_id).await?",
  "pg_advisory_xact_lock",
  'format!("blog-category-tree:{tenant_id}")',
  "child_depth(parent.depth, parent_id)?",
]);

requireMarkers("crates/rustok-blog/src/controllers/categories.rs", [
  "CategoryCommandService::new(runtime.db_clone(), runtime.event_bus())",
  'path = "/api/blog/categories/{id}/move"',
  "request_body = MoveCategoryInput",
  "ensure_category_permission(&auth, Action::Manage)?",
  ".move_category(tenant.id, id, security_context(&auth), input)",
]);

requireMarkers("crates/rustok-blog/src/controllers/mod.rs", [
  '"/api/blog/categories/{id}/move"',
  "post(categories::move_category)",
]);

requireMarkers("crates/rustok-blog/src/openapi.rs", [
  "crate::controllers::categories::move_category",
  "crate::dto::MoveCategoryInput",
  "crate::dto::CategoryPlacementResponse",
  "crate::dto::MoveCategoryResponse",
]);

requireMarkers("crates/rustok-blog/src/migrations/m20260812_000017_enforce_blog_category_hierarchy.rs", [
  "fk_blog_categories_tenant_parent",
  "validate_and_compute_depths",
  "rejects_cross_tenant_parent",
  "rejects_cycle",
]);

requireMarkers("crates/rustok-blog/tests/category_hierarchy.rs", [
  "create_inserts_at_dense_sibling_index_and_rejects_out_of_range_position",
  "create position must be an insertion index inside the sibling list",
  "move_reparents_subtree_and_failed_moves_leave_tree_unchanged",
  "localized update must not be a second hierarchy placement write path",
  "child should move under the second root",
  "child should move to the root level",
  "a category cannot become its own parent",
  "a category cannot move beneath its own descendant",
  "cross-tenant parent must be rejected",
]);

requireMarkers("crates/rustok-blog/src/translation_target_tests.rs", [
  "category_update_advances_exact_locale_and_owner_change_revisions",
  "position: None",
  "assert_eq!(updated.position, 0)",
]);

requireMarkers("crates/rustok-blog/docs/category-hierarchy-contract.md", [
  "Blog owns its category hierarchy",
  "POST /api/blog/categories/{id}/move",
  "zero-based insertion index",
  "maximum 512 nodes",
  "one owner-side write path",
  "recomputes materialized `depth`",
  "does not rewrite localized category rows",
]);

if (failures.length > 0) {
  console.error("Blog category hierarchy command verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Blog category hierarchy command verification passed");
