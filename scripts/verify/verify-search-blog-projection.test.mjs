#!/usr/bin/env node

/**
 * Fixture test for `verify-search-blog-projection.mjs`.
 *
 * The guard is only useful if it fails for the right reason, so each case copies the
 * real files into a temporary tree, mutates exactly one invariant and asserts the
 * diagnostic. The invariants under test are the ones that moved from Search-side SQL
 * into the Blog owner source: tag source, author source, indexed body, and corpus.
 *
 * Run: `node scripts/verify/verify-search-blog-projection.test.mjs`
 */

import test from "node:test";
import assert from "node:assert/strict";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const repositoryRoot = path.resolve(".");
const verifier = path.resolve("scripts/verify/verify-search-blog-projection.mjs");
const ownerSourcePath = "crates/modules/rustok-blog/src/services/search_projection.rs";
const ingestionPath = "crates/modules/rustok-search/src/ingestion.rs";
const evidencePath =
  "crates/modules/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json";
const files = [
  ownerSourcePath,
  "crates/modules/rustok-search/src/projection_driver.rs",
  ingestionPath,
  "crates/modules/rustok-search/tests/blog_ingestion_contract_test.rs",
  "crates/modules/rustok-search/tests/blog_projection_postgres_test.rs",
  evidencePath,
  "crates/modules/rustok-search/docs/implementation-plan.md",
];

function absolute(root, relativePath) {
  return path.join(root, relativePath);
}

function write(root, relativePath, content) {
  const target = absolute(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function fixture(mutator = () => {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-search-blog-projection-"));
  for (const relativePath of files) {
    const target = absolute(root, relativePath);
    mkdirSync(path.dirname(target), { recursive: true });
    cpSync(path.join(repositoryRoot, relativePath), target);
  }
  mutator(root);
  return root;
}

function run(root) {
  return spawnSync(process.execPath, [verifier], {
    cwd: repositoryRoot,
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

function rejects(mutator) {
  const root = fixture(mutator);
  try {
    return run(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

function rewrite(root, relativePath, mutate) {
  const source = readFileSync(absolute(root, relativePath), "utf8");
  write(root, relativePath, mutate(source));
}

test("search Blog projection verifier accepts the owner-produced projection", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /search Blog projection verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("rejects metadata tags as the projection source", () => {
  const result = rejects((root) =>
    rewrite(root, ownerSourcePath, (source) =>
      source.replace(
        "let tag_map = load_post_tags_map(",
        "let tag_map = load_metadata_tags(p.metadata -> 'tags', ",
      ),
    ),
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /p\.metadata -> 'tags'/);
});

test("rejects an author name joined from the account table", () => {
  const result = rejects((root) =>
    rewrite(root, ownerSourcePath, (source) =>
      source.replace(
        "let author_name = author_names.get(&post.author_id).cloned();",
        "let author_name = load_user_name(&self.db, tenant_id, post.author_id).await?; // LEFT JOIN users u",
      ),
    ),
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /LEFT JOIN users/);
});

test("rejects an excerpt-only search body", () => {
  const result = rejects((root) =>
    rewrite(root, ownerSourcePath, (source) =>
      source.replace(
        "crate::richtext::project_stored_article(&translation.body)",
        "Ok::<_, Error>(((), translation.excerpt.clone().unwrap_or_default()))",
      ),
    ),
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /crate::richtext::project_stored_article\(/);
});

test("rejects a corpus that stops requiring the publication stamp", () => {
  const result = rejects((root) =>
    rewrite(root, ownerSourcePath, (source) =>
      source.replaceAll(
        "blog_post::Column::PublishedAt.is_not_null()",
        "blog_post::Column::Id.is_not_null()",
      ),
    ),
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /blog_post::Column::PublishedAt\.is_not_null\(\)/);
});

test("rejects ingestion that goes back to a private Blog projector", () => {
  const result = rejects((root) =>
    rewrite(root, ingestionPath, (source) =>
      source.replace(
        "async fn refresh_blog_author_scope(",
        "async fn legacy_blog_projector_path(&self) { self.blog_projector.rebuild_tenant(); }\n\n    async fn refresh_blog_author_scope(",
      ),
    ),
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /blog_projector/);
});

test("rejects stale evidence claiming the account table is the author source", () => {
  const result = rejects((root) => {
    const value = JSON.parse(readFileSync(absolute(root, evidencePath), "utf8"));
    value.production_contract.blog_author_account_table_join = true;
    write(root, evidencePath, `${JSON.stringify(value, null, 2)}\n`);
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Blog author projection source drift/);
});

test("rejects stale evidence claiming metadata is canonical", () => {
  const result = rejects((root) => {
    const value = JSON.parse(readFileSync(absolute(root, evidencePath), "utf8"));
    value.production_contract.legacy_metadata_tags_are_projection_source = true;
    write(root, evidencePath, `${JSON.stringify(value, null, 2)}\n`);
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /canonical Blog tag projection source drift/);
});
