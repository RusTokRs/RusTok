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

requireMarkers("crates/rustok-blog/src/dto/category_command.rs", [
  "pub struct MoveCategoryInput",
  "pub parent_id: Option<Uuid>",
  "pub position: u32",
  "pub struct CategoryPlacementResponse",
  "pub depth: i32",
  "MAX_BLOG_CATEGORY_TREE_NODES",
]);

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
  "move_reparents_subtree_and_failed_moves_leave_tree_unchanged",
  "child should move under the second root",
  "child should move to the root level",
  "a category cannot become its own parent",
  "a category cannot move beneath its own descendant",
  "cross-tenant parent must be rejected",
]);

requireMarkers("crates/rustok-blog/docs/category-hierarchy-contract.md", [
  "Blog owns its category hierarchy",
  "POST /api/blog/categories/{id}/move",
  "maximum 512 nodes",
  "recomputes materialized `depth`",
  "does not rewrite localized category rows",
]);

if (failures.length > 0) {
  console.error("Blog category hierarchy command verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Blog category hierarchy command verification passed");
