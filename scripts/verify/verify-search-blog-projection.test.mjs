#!/usr/bin/env node

import test from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const verifier = path.resolve("scripts/verify/verify-search-blog-projection.mjs");

function write(root, relativePath, content) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function fixture({ hardcodedPublic = false, missingPostgresHarness = false } = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-search-blog-projection-"));
  const tablePrefix = hardcodedPublic ? "public." : "";
  write(
    root,
    "crates/rustok-search/src/blog_projector.rs",
    `
      to_regclass('${tablePrefix}blog_posts')
      to_regclass('${tablePrefix}blog_post_translations')
      to_regclass('${tablePrefix}blog_post_channel_visibility')
      to_regclass('${tablePrefix}blog_category_translations')
      FROM blog_posts p
      INSERT INTO search_documents
      DELETE FROM search_documents
    `,
  );
  write(
    root,
    "crates/rustok-search/tests/blog_ingestion_contract_test.rs",
    `
      DomainEvent::BlogPostCreated
      DomainEvent::BlogPostPublished
      DomainEvent::BlogPostUnpublished
      DomainEvent::BlogPostUpdated
      DomainEvent::BlogPostArchived
      DomainEvent::BlogPostDeleted
      target_type: "blog".to_string()
    `,
  );
  if (!missingPostgresHarness) {
    write(
      root,
      "crates/rustok-search/tests/blog_projection_postgres_test.rs",
      `
        RUSTOK_SEARCH_TEST_DATABASE_URL
        SearchModule.migrations()
        SearchIngestionHandler::new
        ContractEventEnvelope::new
        SET search_path TO "{schema_name}", public
        full_blog_reindex_replaces_only_current_tenant_blog_documents
        blog_events_upsert_publish_archive_and_delete_search_document
      `,
    );
  }
  write(
    root,
    "crates/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json",
    JSON.stringify({
      schema_version: 1,
      module: "search",
      surface: "blog_post_projection",
      status: "executable_no_run",
      compile_policy: "not_run_by_request",
      test_targets: [
        "crates/rustok-search/tests/blog_ingestion_contract_test.rs",
        "crates/rustok-search/tests/blog_projection_postgres_test.rs",
      ],
    }),
  );
  write(
    root,
    "crates/rustok-search/docs/implementation-plan.md",
    "search-blog-projection-postgres-harness.json RUSTOK_SEARCH_TEST_DATABASE_URL search_path",
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

test("search Blog projection verifier passes canonical fixture", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /search Blog projection verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("search Blog projection verifier rejects hardcoded public source tables", () => {
  const root = fixture({ hardcodedPublic: true });
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /forbidden to_regclass\('public\.blog_/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("search Blog projection verifier rejects missing PostgreSQL harness", () => {
  const root = fixture({ missingPostgresHarness: true });
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /expected file is missing|missing RUSTOK_SEARCH_TEST_DATABASE_URL/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
