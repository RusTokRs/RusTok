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

function fixture({ missingOutbox = false, unboundedPagination = false } = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-blog-category-reindex-"));
  const servicePath = "crates/rustok-blog/src/services/category.rs";
  const controllerPath = "crates/rustok-blog/src/controllers/categories.rs";
  const routerPath = "crates/rustok-blog/src/controllers/mod.rs";
  const openapiPath = "crates/rustok-blog/src/openapi.rs";
  const projectorPath = "crates/rustok-search/src/blog_projector.rs";
  const ingestionPath = "crates/rustok-search/src/ingestion.rs";
  const evidencePath =
    "crates/rustok-blog/contracts/evidence/blog-category-search-reindex-contract.json";

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
    `,
  );
  write(
    root,
    controllerPath,
    `
      CategoryService::new_with_event_bus
      runtime.event_bus()
      filter.page = filter.page.max(1)
      ${unboundedPagination ? "filter.per_page = filter.per_page.max(1)" : "filter.per_page = filter.per_page.clamp(1, 100)"}
      ensure_category_permission
      Resource::Categories
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
        owner_service: servicePath,
        http_adapter: controllerPath,
        router: routerPath,
        openapi: openapiPath,
        search_projector: projectorPath,
        search_ingestion: ingestionPath,
      },
    }),
  );
  write(
    root,
    "crates/rustok-blog/docs/implementation-plan.md",
    "blog-category-search-reindex-contract.json verify-blog-category-search-reindex.mjs category_name category_slug",
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

test("Blog category reindex verifier accepts the owner contract", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("Blog category reindex verifier rejects missing transactional event", () => {
  const root = fixture({ missingOutbox: true });
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /missing DomainEvent::ReindexRequested/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("Blog category reindex verifier rejects unbounded HTTP pagination", () => {
  const root = fixture({ unboundedPagination: true });
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /missing filter.per_page = filter.per_page.clamp\(1, 100\)/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
