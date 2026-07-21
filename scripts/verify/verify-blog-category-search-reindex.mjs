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

function requireBeforeWithin(source, startMarker, endMarker, before, after, label) {
  const start = source.indexOf(startMarker);
  const end = source.indexOf(endMarker, start + startMarker.length);
  if (start < 0 || end < 0) {
    failures.push(`${label}: unable to locate bounded source section`);
    return;
  }
  const section = source.slice(start, end);
  const beforeIndex = section.indexOf(before);
  const afterIndex = section.indexOf(after);
  if (beforeIndex < 0 || afterIndex < 0 || beforeIndex >= afterIndex) {
    failures.push(`${label}: expected ${before} before ${after}`);
  }
}

const permissionPath = "crates/rustok-api/src/permissions.rs";
const platformRbacPath = "crates/rustok-core/src/rbac.rs";
const oauthPath = "crates/rustok-api/src/context/auth.rs";
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

const permissionSource = read(permissionPath);
const platformRbac = read(platformRbacPath);
const oauth = read(oauthPath);
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
  "BlogCategories",
  'Self::BlogCategories => "blog_categories"',
  '"blog_categories" => Ok(Self::BlogCategories)',
  "BLOG_CATEGORIES_CREATE => (BlogCategories, Create)",
  "BLOG_CATEGORIES_READ => (BlogCategories, Read)",
  "BLOG_CATEGORIES_UPDATE => (BlogCategories, Update)",
  "BLOG_CATEGORIES_DELETE => (BlogCategories, Delete)",
  "BLOG_CATEGORIES_LIST => (BlogCategories, List)",
  "BLOG_CATEGORIES_MANAGE => (BlogCategories, Manage)",
]) {
  requireMarker(permissionSource, marker, permissionPath);
}

for (const marker of [
  "Resource::BlogCategories",
  "Permission::BLOG_CATEGORIES_CREATE",
  "Permission::BLOG_CATEGORIES_READ",
  "Permission::BLOG_CATEGORIES_UPDATE",
  "Permission::BLOG_CATEGORIES_DELETE",
  "Permission::BLOG_CATEGORIES_LIST",
  "Permission::BLOG_CATEGORIES_MANAGE",
  "catalog_category_permission_does_not_authorize_blog_categories",
  "security.get_scope(Resource::BlogCategories, Action::Update)",
]) {
  requireMarker(platformRbac, marker, platformRbacPath);
}

for (const marker of [
  "Resource::BlogCategories",
  "blog_categories_are_content_not_catalog",
  "storefront_scope_admits_blog_category_reads",
]) {
  requireMarker(oauth, marker, oauthPath);
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
  "Resource::BlogCategories, Action::Create",
  "Resource::BlogCategories, Action::Read",
  "Resource::BlogCategories, Action::Update",
  "Resource::BlogCategories, Action::Delete",
  "Resource::BlogCategories, Action::List",
]) {
  requireMarker(service, marker, servicePath);
}
for (const marker of [
  "CATEGORY_PERMISSION_RESOURCES",
  "enforce_any_scope",
  "Resource::BlogPosts",
  "Resource::Categories",
  "enforce_owned_scope",
]) {
  rejectMarker(service, marker, servicePath);
}
requireBeforeWithin(
  service,
  "pub async fn update(",
  "pub async fn delete(",
  "enforce_scope(",
  "let txn = self.db.begin().await",
  `${servicePath}: update authorization order`,
);
requireBeforeWithin(
  service,
  "pub async fn delete(",
  "pub async fn list(",
  "enforce_scope(",
  "let txn = self.db.begin().await",
  `${servicePath}: delete authorization order`,
);

requireMarker(rbac, "pub(crate) fn enforce_scope", rbacPath);
for (const marker of ["enforce_any_scope", "primary_or_legacy", "legacy_resource"]) {
  rejectMarker(rbac, marker, rbacPath);
}

for (const marker of [
  "Permission::BLOG_POSTS_MANAGE",
  "Permission::BLOG_CATEGORIES_CREATE",
  "Permission::BLOG_CATEGORIES_READ",
  "Permission::BLOG_CATEGORIES_UPDATE",
  "Permission::BLOG_CATEGORIES_DELETE",
  "Permission::BLOG_CATEGORIES_LIST",
  "Permission::BLOG_CATEGORIES_MANAGE",
  "p.resource == Resource::BlogCategories",
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
  "Permission::new(Resource::BlogCategories, action)",
  "has_effective_permission(&auth.permissions, &permission)",
  "fn map_category_error",
  "BlogError::CategoryNotFound",
  "HttpError::not_found",
  "HttpError::internal",
]) {
  requireMarker(controller, marker, controllerPath);
}
for (const marker of [
  "Resource::BlogPosts",
  "Resource::Categories",
  "has_any_effective_permission",
  "primary",
  "legacy",
]) {
  rejectMarker(controller, marker, controllerPath);
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
  if (contract.permission_resource !== "blog_categories") {
    failures.push(`${evidencePath}: permission_resource must be blog_categories`);
  }
  for (const [key, expected] of Object.entries({
    owner_service: servicePath,
    owner_rbac: rbacPath,
    platform_permissions: permissionPath,
    platform_rbac: platformRbacPath,
    oauth_scope_policy: oauthPath,
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
    "dedicated_permission_namespace",
    "category_has_no_owner_scope",
    "authorization_precedes_lookup",
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
  "blog_categories:*",
]) {
  requireMarker(plan, marker, planPath);
}
for (const marker of ["Legacy `categories:*`", "compatibility fallback", "temporary legacy"]) {
  rejectMarker(plan, marker, planPath);
}

if (failures.length > 0) {
  console.error("Blog category search reindex verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Blog category search reindex verification passed");
