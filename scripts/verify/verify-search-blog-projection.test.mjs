#!/usr/bin/env node

import test from "node:test";
import assert from "node:assert/strict";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const repositoryRoot = path.resolve(".");
const verifier = path.resolve("scripts/verify/verify-search-blog-projection.mjs");
const files = [
  "crates/modules/rustok-search/src/blog_projector.rs",
  "crates/modules/rustok-search/src/ingestion.rs",
  "crates/modules/rustok-search/tests/blog_ingestion_contract_test.rs",
  "crates/modules/rustok-search/tests/blog_projection_postgres_test.rs",
  "crates/modules/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json",
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

test("search Blog projection verifier accepts canonical owner-tag source", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /search Blog projection verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("rejects an author join without the Blog tenant predicate", () => {
  const result = rejects((root) => {
    const relativePath = "crates/modules/rustok-search/src/blog_projector.rs";
    const source = readFileSync(absolute(root, relativePath), "utf8");
    write(
      root,
      relativePath,
      source.replace("\n               AND u.tenant_id = p.tenant_id", ""),
    );
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /u\.tenant_id = p\.tenant_id|tenant-scoped/);
});

test("rejects metadata tags as Search projection source", () => {
  const result = rejects((root) => {
    const relativePath = "crates/modules/rustok-search/src/blog_projector.rs";
    const source = readFileSync(absolute(root, relativePath), "utf8");
    write(root, relativePath, source.replace("FROM blog_post_tags relation", "FROM jsonb_array_elements_text(p.metadata -> 'tags') relation"));
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /jsonb_array_elements_text|blog_post_tags/);
});

test("rejects missing Taxonomy table availability gate", () => {
  const result = rejects((root) => {
    const relativePath = "crates/modules/rustok-search/src/blog_projector.rs";
    const source = readFileSync(absolute(root, relativePath), "utf8");
    write(root, relativePath, source.replace("AND to_regclass('taxonomy_term_translations') IS NOT NULL", ""));
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /taxonomy_term_translations/);
});

test("rejects destructive Blog projection before source schema validation", () => {
  const result = rejects((root) => {
    const relativePath = "crates/modules/rustok-search/src/blog_projector.rs";
    const source = readFileSync(absolute(root, relativePath), "utf8");
    write(
      root,
      relativePath,
      source.replaceAll("self.ensure_blog_tables_available(&tx).await?;\n", ""),
    );
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /validate source schema before destructive deletion|ensure_blog_tables_available/);
});

test("rejects Blog projection schema validation moved after deletion", () => {
  const result = rejects((root) => {
    const relativePath = "crates/modules/rustok-search/src/blog_projector.rs";
    const source = readFileSync(absolute(root, relativePath), "utf8");
    write(
      root,
      relativePath,
      source
        .replace(
          "self.ensure_blog_tables_available(&tx).await?;\n            self.delete_tenant_documents_in",
          "self.delete_tenant_documents_in",
        )
        .replace(
          "self.delete_tenant_documents_in(&tx, tenant_id).await?;\n            self.upsert_documents_in",
          "self.delete_tenant_documents_in(&tx, tenant_id).await?;\n            self.ensure_blog_tables_available(&tx).await?;\n            self.upsert_documents_in",
        ),
    );
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /validate source schema before destructive deletion/);
});

test("rejects silent Blog schema availability decode fallback", () => {
  const result = rejects((root) => {
    const relativePath = "crates/modules/rustok-search/src/blog_projector.rs";
    const source = readFileSync(absolute(root, relativePath), "utf8");
    write(
      root,
      relativePath,
      source
        .replace(
          '.try_get::<bool>("", "available")\n            .map_err(Error::Database)?;',
          '.try_get::<bool>("", "available").ok().unwrap_or(false);',
        ),
    );
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /\.map_err\(Error::Database\)\?/);
});

test("rejects stale evidence claiming metadata is canonical", () => {
  const result = rejects((root) => {
    const relativePath = "crates/modules/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json";
    const value = JSON.parse(readFileSync(absolute(root, relativePath), "utf8"));
    value.production_contract.legacy_metadata_tags_are_projection_source = true;
    write(root, relativePath, `${JSON.stringify(value, null, 2)}\n`);
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /canonical Blog tag projection source drift/);
});
