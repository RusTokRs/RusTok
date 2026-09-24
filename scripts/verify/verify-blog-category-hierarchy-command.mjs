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
    if (!source.includes(marker)) {
      failures.push(`${relative}: missing ${marker}`);
    }
  }
  return source;
}

function rejectMarkers(relative, markers) {
  const source = read(relative);
  for (const marker of markers) {
    if (source.includes(marker)) {
      failures.push(`${relative}: forbidden ${marker}`);
    }
  }
  return source;
}

function segment(source, startMarker, endMarker) {
  const start = source.indexOf(startMarker);
  if (start < 0) return "";
  const end = endMarker ? source.indexOf(endMarker, start + startMarker.length) : -1;
  return source.slice(start, end < 0 ? source.length : end);
}

const dto = requireMarkers("crates/modules/rustok-blog/src/dto/category.rs", [
  "pub struct CreateCategoryInput",
  "pub parent_id: Option<Uuid>",
  "pub position: Option<i32>",
  "pub struct UpdateCategoryInput",
  "pub settings: Option<serde_json::Value>",
  "pub struct CategoryResponse",
]);
const updateDto = segment(dto, "pub struct UpdateCategoryInput", "pub struct CategoryResponse");
if (updateDto.includes("position:")) {
  failures.push("crates/modules/rustok-blog/src/dto/category.rs: UpdateCategoryInput must not contain hierarchy position");
}
rejectMarkers("crates/modules/rustok-blog/src/dto/category.rs", [
  "Compatibility field retained for decoding only",
  "compatibility decoding",
]);

requireMarkers("crates/modules/rustok-blog/src/dto/category_command.rs", [
  "pub struct MoveCategoryInput",
  "pub parent_id: Option<Uuid>",
  "pub position: u32",
  "pub struct CategoryPlacementResponse",
  "pub depth: i32",
  "MAX_BLOG_CATEGORY_TREE_NODES",
]);

const categoryService = requireMarkers("crates/modules/rustok-blog/src/services/category.rs", [
  "lock_category_hierarchy_writer_in_tx(&txn, tenant_id).await?",
  "ensure_category_tree_capacity_in_tx(&txn, tenant_id).await?",
  "ensure_hierarchy_coverage_in_tx(&txn, tenant_id).await?",
  "category_taxonomy_sync::sync_category_copy_in_tx(",
  "canonicalize_siblings_for_insert_in_tx(",
  "publish_blog_reindex_in_tx(&txn, tenant_id, security.user_id)",
  "Blog category tree cannot exceed",
  "Category position cannot be negative",
  "normalize_term_route_key",
]);
const updateBody = segment(categoryService, "pub async fn update(", "pub(crate) async fn ensure_exists_in_tx");
if (updateBody.includes("input.position") || updateBody.includes("Category position is structural")) {
  failures.push("crates/modules/rustok-blog/src/services/category.rs: localized update must not accept or reject structural position");
}
rejectMarkers("crates/modules/rustok-blog/src/services/category.rs", [
  "lock_category_tree_in_tx",
  "format!("blog-category-tree:{tenant_id}")",
  "pub async fn delete(",
]);

const commandService = requireMarkers("crates/modules/rustok-blog/src/services/category_command.rs", [
  "pub struct CategoryCommandService",
  "pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self",
  "Resource::BlogCategories, Action::Manage",
  "lock_category_hierarchy_writer_in_tx(&txn, tenant_id).await?",
  "TaxonomyOwnerCategoryReader::load_scoped_categories_in_strict(",
  "reorder_module_category_siblings_in_tx(",
  "validate_and_compute_depths(&parent_by_id)",
  "parent_by_id.insert(category_id, input.parent_id)",
  "persist_sibling_order(",
  "persist_descendant_depth_changes(",
  "DomainEvent::ReindexRequested",
  "self.event_bus",
  "publish_in_tx(",
  "txn.commit().await?",
]);
rejectMarkers("crates/modules/rustok-blog/src/services/category_command.rs", [
  "lock_category_tree_in_tx",
  "pg_advisory_xact_lock",
]);

const deleteCleanup = requireMarkers("crates/modules/rustok-blog/src/services/category_delete.rs", [
  "impl TaxonomyCategoryDeleteCleanupPort for BlogCategoryDeleteCleanup",
  "lock_category_hierarchy_writer_in_tx(txn, tenant_id).await?",
  "delete_module_category_placement_and_compact_in_tx(",
  "detach_category_from_posts_in_tx(",
  "blog_category::Entity::delete_many()",
  "blog_category::Column::Revision.eq(category.revision)",
  "publish_in_tx(",
  "self.capability_cleanup",
]);
rejectMarkers("crates/modules/rustok-blog/src/services/category_delete.rs", [
  "ensure_category_is_leaf_in_tx",
  "canonicalize_siblings_in_tx",
  "blog_category_translation::",
]);

requireMarkers("crates/modules/rustok-blog/src/services/category_owner.rs", [
  "TaxonomyService::new(self.db.clone())",
  ".delete_module_category_with_cleanup(",
  "BlogCategoryDeleteCleanup::new(",
  "Blog Category delete requires host-composed Taxonomy capability cleanup",
]);

const entity = requireMarkers("crates/modules/rustok-blog/src/entities/blog_category.rs", [
  "pub struct Model",
  "pub tenant_id: Uuid",
  "pub settings: Json",
  "pub revision: i64",
]);
rejectMarkers("crates/modules/rustok-blog/src/entities/blog_category.rs", [
  "parent_id",
  "position",
  "depth",
]);

requireMarkers("crates/modules/rustok-blog/src/migrations/m20260916_000022_clean_blog_category_canonical_taxonomy.rs", [
  "Taxonomy is the canonical owner of Category hierarchy",
  ".drop_column(BlogCategories::ParentId)",
  ".drop_column(BlogCategories::Position)",
  ".drop_column(BlogCategories::Depth)",
  "Intentionally irreversible under Zero-Legacy Policy",
]);

requireMarkers("crates/modules/rustok-blog/src/controllers/categories.rs", [
  "CategoryCommandService::new(runtime.db_clone(), runtime.event_bus())",
  "path = "/api/blog/categories/{id}/move"",
  "request_body = MoveCategoryInput",
  "ensure_category_permission(&tenant, &auth, Action::Manage)?;",
  ".move_category(tenant.id, id, security_context(&auth), input)",
]);

requireMarkers("crates/modules/rustok-blog/src/controllers/mod.rs", [
  '"/api/blog/categories/{id}/move"',
  "post(categories::move_category)",
]);

requireMarkers("crates/modules/rustok-blog/src/controllers/openapi.rs", [
  "crate::controllers::categories::move_category",
  "crate::dto::MoveCategoryInput",
  "crate::dto::CategoryPlacementResponse",
  "crate::dto::MoveCategoryResponse",
]);

const hierarchyTests = requireMarkers("crates/modules/rustok-blog/tests/category_hierarchy.rs", [
  "CategoryCommandService::new(db.clone(), event_bus)",
  "create_inserts_at_dense_sibling_index_and_rejects_out_of_range_position",
  "move_reparents_subtree_and_failed_moves_leave_tree_unchanged",
  "localized update must not be a second hierarchy placement write path",
  "child should move under the second root",
  "child should move to the root level",
  "a category cannot become its own parent",
  "a category cannot move beneath its own descendant",
  "cross-tenant parent must be rejected",
  "delete_rejects_non_leaf_and_compacts_remaining_siblings",
]);
if (hierarchyTests.includes("position: Some(7)") || hierarchyTests.includes("Compatibility field retained for decoding only")) {
  failures.push("crates/modules/rustok-blog/tests/category_hierarchy.rs: stale UpdateCategoryInput compatibility field usage");
}

requireMarkers("crates/modules/rustok-blog/tests/category_taxonomy_delete_lifecycle.rs", [
  "delete_removes_blog_binding_and_taxonomy_owner_and_replays_sibling_position",
  "host_cleanup_failure_rolls_back_blog_and_taxonomy_deletion",
]);

requireMarkers("crates/modules/rustok-blog/tests/category_taxonomy_mutation_response_cutover.rs", [
  "update_response_comes_from_taxonomy_without_requiring_read_permission",
]);
const responseCutover = read("crates/modules/rustok-blog/tests/category_taxonomy_mutation_response_cutover.rs");
if (responseCutover.includes("position: None")) {
  failures.push("crates/modules/rustok-blog/tests/category_taxonomy_mutation_response_cutover.rs: stale UpdateCategoryInput.position fixture field");
}

requireMarkers("crates/modules/rustok-blog/docs/category-hierarchy-contract.md", [
  "Taxonomy owns canonical Blog Category hierarchy placement",
  "UpdateCategoryInput contains no hierarchy fields",
  "POST /api/blog/categories/{id}/move",
  "zero-based insertion index",
  "maximum of 512 nodes",
  "leaf-only",
  "ON DELETE RESTRICT",
  "projection-neutral",
  "compacts remaining sibling positions",
  "one owner-side write path",
  "recompute response `depth` from the canonical Taxonomy parent map",
  "Structural moves do not rewrite localized category rows",
]);

rejectMarkers("crates/modules/rustok-blog/docs/category-hierarchy-contract.md", [
  "retained only for compatibility",
  "compatibility decoding",
]);

if (failures.length > 0) {
  console.error("Blog category hierarchy current-contract verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Blog category hierarchy current-contract verification passed");
