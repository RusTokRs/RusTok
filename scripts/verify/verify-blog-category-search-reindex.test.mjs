#!/usr/bin/env node

import test from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const verifier = path.resolve("scripts/verify/verify-blog-category-search-reindex.mjs");

function write(root, relativePath, content) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function fixture({
  missingOutbox = false,
  missingSlugGuard = false,
  unboundedServicePagination = false,
  unboundedHttpPagination = false,
  missingTypedErrors = false,
  ownedScopeRegression = false,
  lookupBeforeAuthorization = false,
  wrongServiceResource = false,
  wrongHttpResource = false,
  legacyHelperRegression = false,
  advertisesCatalogPermissions = false,
  catalogOauthLeak = false,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-blog-category-reindex-"));
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

  write(
    root,
    permissionPath,
    `
      BlogCategories,
      Self::BlogCategories => "blog_categories"
      "blog_categories" => Ok(Self::BlogCategories)
      BLOG_CATEGORIES_CREATE => (BlogCategories, Create)
      BLOG_CATEGORIES_READ => (BlogCategories, Read)
      BLOG_CATEGORIES_UPDATE => (BlogCategories, Update)
      BLOG_CATEGORIES_DELETE => (BlogCategories, Delete)
      BLOG_CATEGORIES_LIST => (BlogCategories, List)
      BLOG_CATEGORIES_MANAGE => (BlogCategories, Manage)
    `,
  );

  write(
    root,
    platformRbacPath,
    `
      Resource::BlogCategories
      Permission::BLOG_CATEGORIES_CREATE
      Permission::BLOG_CATEGORIES_READ
      Permission::BLOG_CATEGORIES_UPDATE
      Permission::BLOG_CATEGORIES_DELETE
      Permission::BLOG_CATEGORIES_LIST
      Permission::BLOG_CATEGORIES_MANAGE
      catalog_category_permission_does_not_authorize_blog_categories
      security.get_scope(Resource::BlogCategories, Action::Update)
    `,
  );

  write(
    root,
    oauthPath,
    `
      Resource::BlogCategories
      blog_categories_are_content_not_catalog
      storefront_scope_admits_blog_category_reads
      ${catalogOauthLeak ? '"catalog" => matches!(resource, Resource::BlogCategories)' : ""}
    `,
  );

  const updatePermission = wrongServiceResource
    ? "enforce_scope(&security, Resource::BlogPosts, Action::Update)?;"
    : "enforce_scope(&security, Resource::BlogCategories, Action::Update)?;";
  const deletePermission = "enforce_scope(&security, Resource::BlogCategories, Action::Delete)?;";
  const transaction = "let txn = self.db.begin().await;";
  const updateOrdered = lookupBeforeAuthorization
    ? `${transaction}\n${updatePermission}`
    : `${updatePermission}\n${transaction}`;
  const deleteOrdered = lookupBeforeAuthorization
    ? `${transaction}\n${deletePermission}`
    : `${deletePermission}\n${transaction}`;

  write(
    root,
    servicePath,
    `
      event_bus: Option<TransactionalEventBus>
      pub fn new_with_event_bus() {}
      self.db.begin().await
      publish_blog_reindex_in_tx
      ${missingOutbox ? "" : "DomainEvent::ReindexRequested"}
      target_type: "blog".to_string()
      target_id: None
      txn.commit().await
      blog_category_translation::Column::TenantId.eq(tenant_id)
      Self::ensure_exists_in_tx(&txn, tenant_id, parent_id).await?
      normalize_category_slug(input.slug.as_deref(), &input.name)?
      ${missingSlugGuard ? "" : "Slug must contain at least one ASCII letter or digit"}
      enforce_scope(&security, Resource::BlogCategories, Action::Create)?;
      enforce_scope(&security, Resource::BlogCategories, Action::Read)?;
      ${ownedScopeRegression ? "enforce_owned_scope" : ""}
      ${legacyHelperRegression ? "enforce_any_scope" : ""}

      pub async fn update() {
        ${updateOrdered}
      }

      pub async fn delete() {
        ${deleteOrdered}
      }

      pub async fn list() {
        enforce_scope(&security, Resource::BlogCategories, Action::List)?;
        ${unboundedServicePagination ? "let per_page = filter.per_page.max(1)" : "let per_page = filter.per_page.clamp(1, 100)"}
        .paginate(&self.db, per_page)
      }
    `,
  );

  write(
    root,
    rbacPath,
    `
      pub(crate) fn enforce_scope() {}
      ${legacyHelperRegression ? "pub(crate) fn enforce_any_scope() {} primary_or_legacy" : ""}
    `,
  );

  write(
    root,
    modulePath,
    `
      Permission::BLOG_POSTS_MANAGE
      Permission::BLOG_CATEGORIES_CREATE
      Permission::BLOG_CATEGORIES_READ
      Permission::BLOG_CATEGORIES_UPDATE
      Permission::BLOG_CATEGORIES_DELETE
      Permission::BLOG_CATEGORIES_LIST
      Permission::BLOG_CATEGORIES_MANAGE
      p.resource == Resource::BlogCategories
      p.resource == Resource::Categories
      ${advertisesCatalogPermissions ? "Permission::new(Resource::Categories, Action::Update)" : ""}
    `,
  );

  const httpPermission = wrongHttpResource
    ? "Permission::new(Resource::Categories, action)"
    : "Permission::new(Resource::BlogCategories, action)";
  write(
    root,
    controllerPath,
    `
      CategoryService::new_with_event_bus
      runtime.event_bus()
      filter.page = filter.page.max(1)
      ${unboundedHttpPagination ? "filter.per_page = filter.per_page.max(1)" : "filter.per_page = filter.per_page.clamp(1, 100)"}
      ensure_category_permission
      ${httpPermission}
      has_effective_permission(&auth.permissions, &permission)
      ${missingTypedErrors ? "" : "fn map_category_error BlogError::CategoryNotFound HttpError::not_found HttpError::internal"}
    `,
  );

  write(
    root,
    routerPath,
    `
      pub mod categories;
      "/api/blog/categories"
      "/api/blog/categories/{id}"
      categories::update_category
      categories::delete_category
    `,
  );
  write(
    root,
    openapiPath,
    `
      crate::controllers::categories::list_categories
      crate::controllers::categories::create_category
      crate::controllers::categories::update_category
      crate::controllers::categories::delete_category
      crate::dto::CategoryListResponse
    `,
  );
  write(
    root,
    projectorPath,
    `
      'category_name', bct.name
      'category_slug', bct.slug
      LEFT JOIN blog_category_translations bct
    `,
  );
  write(
    root,
    ingestionPath,
    `
      ("blog", None) => self.blog_projector.rebuild_tenant(tenant_id).await
      target_type == "blog"
    `,
  );

  write(
    root,
    evidencePath,
    JSON.stringify({
      schema_version: 1,
      module: "blog",
      surface: "category_search_reindex",
      status: "source_verified_no_compile",
      compile_policy: "not_run_by_request",
      production_contract: {
        permission_resource: "blog_categories",
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
      },
      cases: [
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
      ].map((name) => ({ name })),
    }),
  );
  write(
    root,
    "crates/rustok-blog/docs/implementation-plan.md",
    "blog-category-search-reindex-contract.json verify-blog-category-search-reindex.mjs category_name category_slug non-empty ASCII slug service and HTTP pagination blog_categories:*",
  );

  return root;
}

function run(root) {
  return spawnSync(process.execPath, [verifier], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

function expectRejected(options, pattern) {
  const root = fixture(options);
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("Blog category verifier accepts the dedicated permission contract", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("rejects missing transactional event", () => {
  expectRejected({ missingOutbox: true }, /missing DomainEvent::ReindexRequested/);
});

test("rejects empty-slug regression", () => {
  expectRejected(
    { missingSlugGuard: true },
    /missing Slug must contain at least one ASCII letter or digit/,
  );
});

test("rejects unbounded owner pagination", () => {
  expectRejected(
    { unboundedServicePagination: true },
    /missing let per_page = filter\.per_page\.clamp\(1, 100\)/,
  );
});

test("rejects unbounded HTTP pagination", () => {
  expectRejected(
    { unboundedHttpPagination: true },
    /missing filter\.per_page = filter\.per_page\.clamp\(1, 100\)/,
  );
});

test("rejects flattened HTTP errors", () => {
  expectRejected({ missingTypedErrors: true }, /missing fn map_category_error/);
});

test("rejects category UUID ownership checks", () => {
  expectRejected({ ownedScopeRegression: true }, /forbidden enforce_owned_scope/);
});

test("rejects lookup before authorization", () => {
  expectRejected(
    { lookupBeforeAuthorization: true },
    /expected enforce_scope\( before let txn = self\.db\.begin\(\)\.await/,
  );
});

test("rejects blog_posts authorization for Blog categories", () => {
  expectRejected({ wrongServiceResource: true }, /forbidden Resource::BlogPosts/);
});

test("rejects catalog authorization for Blog categories", () => {
  expectRejected({ wrongHttpResource: true }, /forbidden Resource::Categories/);
});

test("rejects legacy multi-resource helpers", () => {
  expectRejected({ legacyHelperRegression: true }, /forbidden enforce_any_scope/);
});

test("rejects catalog permission advertisement", () => {
  expectRejected(
    { advertisesCatalogPermissions: true },
    /forbidden Permission::new\(Resource::Categories/,
  );
});

test("rejects OAuth catalog leakage", () => {
  expectRejected({ catalogOauthLeak: true }, /verification failed/);
});
