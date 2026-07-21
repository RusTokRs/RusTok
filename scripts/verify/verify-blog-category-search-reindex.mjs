#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

function read(relativePath) {
  const target = path.join(repoRoot, relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(target, "utf8");
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function rejectMarker(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

const servicePath = "crates/rustok-blog/src/services/category.rs";
const rbacPath = "crates/rustok-blog/src/services/rbac.rs";
const modulePath = "crates/rustok-blog/src/lib.rs";
const controllerPath = "crates/rustok-blog/src/controllers/categories.rs";
const routerPath = "crates/rustok-blog/src/controllers/mod.rs";
const openapiPath = "crates/rustok-blog/src/openapi.rs";
const projectorPath = "crates/rustok-search/src/blog_projector.rs";
const ingestionPath = "crates/rustok-search/src/ingestion.rs";
const evidencePath =
  "crates/rustok-blog/contracts/evidence/blog-category-search-reindex-contract.json";
const planPath = "crates/rustok-blog/docs/implementation-plan.md";

const service = read(servicePath);
const rbac = read(rbacPath);
const moduleSource = read(modulePath);
const controller = read(controllerPath);
const router = read(routerPath);
const openapi = read(openapiPath);
const projector = read(projectorPath);
const ingestion = read(ingestionPath);
const plan = read(planPath);
let evidence = null;
try {
  evidence = JSON.parse(read(evidencePath));
} catch (error) {
  failures.push(`${evidencePath}: invalid JSON: ${error.message}`);
}

for (const marker of [
  "pub fn new_with_event_bus",
  "event_bus: Option<TransactionalEventBus>",
  "self.db.begin().await",
  "publish_blog_reindex_in_tx",
  "DomainEvent::ReindexRequested",
  'target_type: "blog".to_string()',
  "target_id: None",
  "txn.commit().await",
  "blog_category_translation::Column::TenantId.eq(tenant_id)",
  "Self::ensure_exists_in_tx(&txn, tenant_id, parent_id).await?",
  "normalize_category_slug(input.slug.as_deref(), &input.name)?",
  "Slug must contain at least one ASCII letter or digit",
  "let per_page = filter.per_page.clamp(1, 100)",
  ".paginate(&self.db, per_page)",
  "CATEGORY_PERMISSION_RESOURCES",
  "[Resource::BlogPosts, Resource::Categories]",
  "enforce_any_scope",
]) {
  requireMarker(service, marker, servicePath);
}
rejectMarker(service, "enforce_owned_scope", servicePath);

for (const marker of [
  "pub(crate) fn enforce_any_scope",
  ".iter()",
  "security.get_scope(*resource, action)",
  "any_scope_accepts_primary_or_legacy_resource",
]) {
  requireMarker(rbac, marker, rbacPath);
}

for (const marker of [
  "Permission::BLOG_POSTS_CREATE",
  "Permission::BLOG_POSTS_READ",
  "Permission::BLOG_POSTS_UPDATE",
  "Permission::BLOG_POSTS_DELETE",
  "Permission::BLOG_POSTS_LIST",
  "Permission::BLOG_POSTS_MANAGE",
  "p.resource == Resource::Categories",
]) {
  requireMarker(moduleSource, marker, modulePath);
}
rejectMarker(moduleSource, "Permission::new(Resource::Categories", modulePath);

for (const marker of [
  "CategoryService::new_with_event_bus",
  "runtime.event_bus()",
  "filter.page = filter.page.max(1)",
  "filter.per_page = filter.per_page.clamp(1, 100)",
  "ensure_category_permission",
  "Permission::new(Resource::BlogPosts, action)",
  "Permission::new(Resource::Categories, action)",
  "has_any_effective_permission(&auth.permissions, &[primary, legacy])",
  "fn map_category_error",
  "BlogError::CategoryNotFound",
  "HttpError::not_found",
  "HttpError::internal",
]) {
  requireMarker(controller, marker, controllerPath);
}

for (const marker of [
  "pub mod categories",
  '"/api/blog/categories"',
  '"/api/blog/categories/{id}"',
  "categories::update_category",
  "categories::delete_category",
]) {
  requireMarker(router, marker, routerPath);
}

for (const marker of [
  "crate::controllers::categories::list_categories",
  "crate::controllers::categories::create_category",
  "crate::controllers::categories::update_category",
  "crate::controllers::categories::delete_category",
  "crate::dto::CategoryListResponse",
]) {
  requireMarker(openapi, marker, openapiPath);
}

for (const marker of [
  "'category_name', bct.name",
  "'category_slug', bct.slug",
  "LEFT JOIN blog_category_translations bct",
]) {
  requireMarker(projector, marker, projectorPath);
}

for (const marker of [
  '("blog", None) => self.blog_projector.rebuild_tenant(tenant_id).await',
  'target_type == "blog"',
]) {
  requireMarker(ingestion, marker, ingestionPath);
}

if (evidence) {
  if (evidence.schema_version !== 1) failures.push(`${evidencePath}: schema_version must be 1`);
  if (evidence.module !== "blog" || evidence.surface !== "category_search_reindex") {
    failures.push(`${evidencePath}: module/surface identity drift`);
  }
  if (evidence.status !== "source_verified_no_compile") {
    failures.push(`${evidencePath}: status drift`);
  }
  if (evidence.compile_policy !== "not_run_by_request") {
    failures.push(`${evidencePath}: compile policy drift`);
  }
  const contract = evidence.production_contract ?? {};
  for (const [key, expected] of Object.entries({
    owner_service: servicePath,
    owner_rbac: rbacPath,
    module_permissions: modulePath,
    http_adapter: controllerPath,
    router: routerPath,
    openapi: openapiPath,
    search_projector: projectorPath,
    search_ingestion: ingestionPath,
  })) {
    if (contract[key] !== expected) failures.push(`${evidencePath}: ${key} path drift`);
  }
  const cases = new Set((evidence.cases ?? []).map((entry) => entry.name));
  for (const requiredCase of [
    "category_update_atomic_reindex",
    "category_delete_atomic_reindex",
    "tenant_scoped_parent",
    "non_empty_slug",
    "bounded_permission_namespace",
    "category_has_no_owner_scope",
    "bounded_category_list",
    "typed_http_errors",
    "search_payload_dependency",
  ]) {
    if (!cases.has(requiredCase)) failures.push(`${evidencePath}: missing case ${requiredCase}`);
  }
}

for (const marker of [
  "blog-category-search-reindex-contract.json",
  "verify-blog-category-search-reindex.mjs",
  "category_name",
  "category_slug",
  "non-empty ASCII slug",
  "service and HTTP pagination",
  "blog_posts:*",
  "Legacy `categories:*`",
]) {
  requireMarker(plan, marker, planPath);
}

if (failures.length > 0) {
  console.error("Blog category search reindex verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Blog category search reindex verification passed");
