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
  return source;
}

function rejectMarkers(relative, markers) {
  const source = read(relative);
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${relative}: forbidden ${marker}`);
  }
  return source;
}

requireMarkers("crates/rustok-blog/src/dto/category_tree.rs", [
  "pub struct CategoryTreeNode",
  "pub parent_id: Option<Uuid>",
  "pub position: i32",
  "pub depth: i32",
  "pub children: Vec<CategoryTreeNode>",
  "pub struct CategoryTreeResponse",
  "pub total_nodes: u32",
  "pub max_depth: i32",
]);

requireMarkers("crates/rustok-blog/src/services/category_tree.rs", [
  "pub struct CategoryTreeService",
  "Resource::BlogCategories, Action::List",
  "MAX_BLOG_CATEGORY_TREE_NODES + 1",
  "resolve_by_locale",
  "available_locales_from",
  "materialized depth",
  "contains a cycle or disconnected hierarchy",
  "missing or foreign parent",
  ".order_by_asc(blog_category::Column::Position)",
  ".order_by_asc(blog_category::Column::Id)",
]);

const categoryQuery = requireMarkers("crates/rustok-blog/src/graphql/category_query.rs", [
  "pub struct BlogCategoryQuery",
  "async fn blog_category(",
  "async fn blog_category_tree(",
  "CategoryService::new",
  "CategoryTreeService::new",
  "tenant.id",
  "resolve_graphql_locale",
]);
for (const forbidden of [
  "blog_category::Entity",
  "blog_category_translation::Entity",
  "tenant_id: Option<Uuid>",
]) {
  if (categoryQuery.includes(forbidden)) {
    failures.push(`crates/rustok-blog/src/graphql/category_query.rs: forbidden ${forbidden}`);
  }
}

const categoryMutation = requireMarkers(
  "crates/rustok-blog/src/graphql/category_mutation.rs",
  [
    "pub struct BlogCategoryMutation",
    "async fn create_blog_category(",
    ") -> Result<Uuid>",
    "async fn update_blog_category(",
    "async fn move_blog_category(",
    "async fn delete_blog_category(",
    "Permission::BLOG_CATEGORIES_CREATE",
    "Permission::BLOG_CATEGORIES_UPDATE",
    "Permission::BLOG_CATEGORIES_MANAGE",
    "Permission::BLOG_CATEGORIES_DELETE",
    "CategoryService::new",
    "CategoryCommandService::new",
    "current_authenticated_tenant",
    "tenant.id != auth.tenant_id",
  ],
);
for (const forbidden of [
  "blog_category::Entity",
  "blog_category_translation::Entity",
  "tenant_id: Option<Uuid>",
]) {
  if (categoryMutation.includes(forbidden)) {
    failures.push(`crates/rustok-blog/src/graphql/category_mutation.rs: forbidden ${forbidden}`);
  }
}
const createStart = categoryMutation.indexOf("async fn create_blog_category(");
const createEnd = categoryMutation.indexOf("async fn update_blog_category(", createStart);
const createBlock =
  createStart >= 0 && createEnd > createStart
    ? categoryMutation.slice(createStart, createEnd)
    : "";
if (createBlock.includes(".get(")) {
  failures.push(
    "crates/rustok-blog/src/graphql/category_mutation.rs: create mutation must not cross a post-commit read permission boundary",
  );
}

const categoryTypes = requireMarkers("crates/rustok-blog/src/graphql/category_types.rs", [
  '#[graphql(name = "UpdateBlogCategoryInput")]',
  '#[graphql(name = "MoveBlogCategoryInput")]',
  "pub parent_id: Option<Uuid>",
  "pub position: u32",
  "position: None",
  "unwrap_or_else(|| serde_json::json!({}))",
]);
const updateStart = categoryTypes.indexOf("pub struct GqlUpdateBlogCategoryInput");
const updateEnd = categoryTypes.indexOf("pub struct GqlMoveBlogCategoryInput", updateStart);
const updateBlock =
  updateStart >= 0 && updateEnd > updateStart
    ? categoryTypes.slice(updateStart, updateEnd)
    : "";
for (const forbidden of ["parent_id", "position"]) {
  if (updateBlock.includes(forbidden)) {
    failures.push(
      `crates/rustok-blog/src/graphql/category_types.rs: localized GraphQL update must not expose ${forbidden}`,
    );
  }
}

requireMarkers("crates/rustok-blog/src/graphql/mod.rs", [
  "use async_graphql::MergedObject",
  "pub struct BlogQuery(query::BlogQuery, category_query::BlogCategoryQuery)",
  "category_mutation::BlogCategoryMutation",
  "category_schema_keeps_localized_and_structural_commands_separate",
  'assert!(!update_input.contains("position:"))',
  'assert!(!update_input.contains("parentId:"))',
]);

requireMarkers("crates/rustok-blog/tests/category_tree.rs", [
  "tree_read_is_ordered_localized_and_rejects_materialized_depth_drift",
  "assert_eq!(tree.roots[0].requested_locale, \"fr\")",
  "assert_eq!(tree.roots[0].effective_locale, \"en\")",
  "fixture should corrupt materialized depth",
  "depth drift must fail closed",
]);

requireMarkers("crates/rustok-blog/docs/category-graphql-contract.md", [
  "Blog owns category hierarchy",
  "CategoryTreeService",
  "512 tenant-local categories",
  "materialized `depth`",
  "current authenticated tenant",
  "UpdateBlogCategoryInput",
  "MoveBlogCategoryInput",
  "does not write owner persistence directly",
  "Blog admin category management is a separate consumer slice",
]);

rejectMarkers("crates/rustok-blog/src/graphql/category_types.rs", [
  "rustok_taxonomy",
  "taxonomy_term",
]);
rejectMarkers("crates/rustok-blog/src/graphql/category_query.rs", [
  "rustok_taxonomy",
  "taxonomy_term",
]);
rejectMarkers("crates/rustok-blog/src/graphql/category_mutation.rs", [
  "rustok_taxonomy",
  "taxonomy_term",
]);

if (failures.length > 0) {
  console.error("Blog category GraphQL contract verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Blog category GraphQL contract verification passed");
