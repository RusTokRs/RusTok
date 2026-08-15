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

function rejectMarkers(relative, markers) {
  const source = read(relative);
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${relative}: forbidden ${marker}`);
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

const categoryService = read("crates/rustok-blog/src/services/category.rs");
requireMarkers("crates/rustok-blog/src/services/category.rs", [
  "lock_category_tree_in_tx(&txn, tenant_id).await?",
  "ensure_category_tree_capacity_in_tx(&txn, tenant_id).await?",
  "canonicalize_siblings_for_insert_in_tx",
  "Category position cannot be negative",
  "exceeds sibling count",
  "Blog category tree cannot exceed",
  "if input.position.is_some()",
  "Category position is structural; use the category move command",
  "ensure_category_is_leaf_in_tx(&txn, tenant_id, category_id).await?",
  "Category must be a leaf before deletion; move or delete its children first",
  "canonicalize_siblings_in_tx",
  'format!("blog-category-tree:{tenant_id}")',
]);

const updateStart = categoryService.indexOf("pub async fn update(");
const updateEnd = categoryService.indexOf("pub async fn delete(", updateStart);
const updateBody =
  updateStart >= 0 && updateEnd > updateStart
    ? categoryService.slice(updateStart, updateEnd)
    : "";
if (updateBody.includes("Column::Position")) {
  failures.push(
    "crates/rustok-blog/src/services/category.rs: localized update must not write hierarchy position",
  );
}

const deleteStart = categoryService.indexOf("pub async fn delete(");
const deleteEnd = categoryService.indexOf("pub async fn list(", deleteStart);
const deleteBody =
  deleteStart >= 0 && deleteEnd > deleteStart
    ? categoryService.slice(deleteStart, deleteEnd)
    : "";
for (const marker of [
  "lock_category_tree_in_tx(&txn, tenant_id).await?",
  "ensure_category_is_leaf_in_tx(&txn, tenant_id, category_id).await?",
  "canonicalize_siblings_in_tx",
  "publish_blog_reindex_in_tx",
  "txn.commit().await",
]) {
  if (!deleteBody.includes(marker)) {
    failures.push(
      `crates/rustok-blog/src/services/category.rs: delete path missing ${marker}`,
    );
  }
}
const leafCheck = deleteBody.indexOf("ensure_category_is_leaf_in_tx");
const deleteExec = deleteBody.indexOf("blog_category::Entity::delete_many()");
if (leafCheck < 0 || deleteExec < 0 || leafCheck > deleteExec) {
  failures.push(
    "crates/rustok-blog/src/services/category.rs: leaf validation must happen before category deletion",
  );
}

requireMarkers("crates/rustok-blog/src/services/category_command.rs", [
  "pub struct CategoryCommandService",
  "pub fn new(db: DatabaseConnection) -> Self",
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
  "txn.commit().await?",
]);
rejectMarkers("crates/rustok-blog/src/services/category_command.rs", [
  "TransactionalEventBus",
  "DomainEvent::ReindexRequested",
  "publish_in_tx",
]);

requireMarkers("crates/rustok-blog/src/entities/blog_category.rs", [
  "lock_category_tree_for_insert(db, tenant_id).await?",
  "pg_advisory_xact_lock",
  'format!("blog-category-tree:{tenant_id}")',
  "child_depth(parent.depth, parent_id)?",
]);

requireMarkers("crates/rustok-blog/src/controllers/categories.rs", [
  "CategoryCommandService::new(runtime.db_clone())",
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
  "ForeignKeyAction::Restrict",
  "validate_and_compute_depths",
  "rejects_cross_tenant_parent",
  "rejects_cycle",
]);

requireMarkers("crates/rustok-blog/tests/category_hierarchy.rs", [
  "CategoryCommandService::new(db.clone())",
  "create_inserts_at_dense_sibling_index_and_rejects_out_of_range_position",
  "create position must be an insertion index inside the sibling list",
  "move_reparents_subtree_and_failed_moves_leave_tree_unchanged",
  "localized update must not be a second hierarchy placement write path",
  "child should move under the second root",
  "child should move to the root level",
  "a category cannot become its own parent",
  "a category cannot move beneath its own descendant",
  "cross-tenant parent must be rejected",
  "delete_rejects_non_leaf_and_compacts_remaining_siblings",
  "a category with children must not be deleted",
  "leaf deletion should succeed",
  "parent should become deletable after all children are removed",
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
  "maximum of 512 nodes",
  "leaf-only",
  "ON DELETE RESTRICT",
  "projection-neutral",
  "compacts remaining sibling positions",
  "one owner-side write path",
  "recompute materialized `depth`",
  "Structural moves do not rewrite localized category rows",
]);

if (failures.length > 0) {
  console.error("Blog category hierarchy command verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Blog category hierarchy command verification passed");
