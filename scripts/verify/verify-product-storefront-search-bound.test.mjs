#!/usr/bin/env node

/**
 * Fixture test for `verify-product-storefront-search-bound.mjs`.
 *
 * The guard is a source-text contract, so it is only trustworthy if it fails for the
 * right reason. Each case copies the real files into a temporary tree, mutates exactly
 * one invariant and asserts the diagnostic:
 *
 *   - the entry point validates the bound before it builds SQL;
 *   - the bound is rejected, never truncated;
 *   - unrelated truncation in the same file is not a violation;
 *   - the owner bound stays exactly representable by the Index `TextLike` pattern.
 *
 * Run: `node scripts/verify/verify-product-storefront-search-bound.test.mjs`
 */

import test from "node:test";
import assert from "node:assert/strict";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const repositoryRoot = path.resolve(".");
const verifier = path.resolve("scripts/verify/verify-product-storefront-search-bound.mjs");
const typesPath = "crates/modules/rustok-product/src/services/catalog/types.rs";
const ownerPath = "crates/modules/rustok-product/src/services/catalog/queries.rs";
const shadowPath = "crates/modules/rustok-distribution/src/product_index/storefront_shadow.rs";
const files = [
  typesPath,
  ownerPath,
  shadowPath,
  "crates/modules/rustok-index/src/application/validation.rs",
  "crates/modules/rustok-product/src/services/catalog.rs",
  "crates/modules/rustok-product/src/services/mod.rs",
];

function write(root, relativePath, content) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function fixture(mutator = () => {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-storefront-search-bound-"));
  for (const relativePath of files) {
    const target = path.join(root, relativePath);
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

function accepts(mutator) {
  const result = rejects(mutator);
  assert.equal(
    result.status,
    0,
    `expected acceptance, got:\n${result.stderr}${result.stdout}`,
  );
}

function rewrite(root, relativePath, mutate) {
  const source = readFileSync(path.join(root, relativePath), "utf8");
  const mutated = mutate(source);
  assert.notEqual(mutated, source, `${relativePath}: fixture mutation changed nothing`);
  write(root, relativePath, mutated);
}

const VALIDATION =
  "types::validate_storefront_product_search(list_query.search.as_deref())?;";
const QUERY = "let mut query = entities::product::Entity::find()";

test("accepts the current owner and the unrelated row truncation", () => {
  const owner = readFileSync(path.join(repositoryRoot, ownerPath), "utf8");
  assert.ok(
    owner.includes("rows.truncate(limit as usize);"),
    "the owner is expected to page rows with a truncation unrelated to search",
  );
  // A comment-only mutation proves the fixture pipeline runs and that the guard does
  // not mistake the keyset scan's `rows.truncate(..)` for a search truncation.
  accepts((root) =>
    rewrite(root, ownerPath, (source) =>
      source.replace(
        "rows.truncate(limit as usize);",
        "// row paging, not search normalization\n        rows.truncate(limit as usize);",
      ),
    ),
  );
});

test("rejects validation placed after the owner query inside the entry point", () => {
  const result = rejects((root) =>
    rewrite(root, ownerPath, (source) => {
      const entry = source.indexOf("pub async fn list_published_products_with_query(");
      assert.ok(entry >= 0, "the Storefront search entry point must exist");
      const before = source.slice(0, entry);
      const body = source.slice(entry);
      return `${before}${body.replace(VALIDATION, "").replace(QUERY, `${QUERY}\n        ${VALIDATION}`)}`
        .replace(`${QUERY}\n        ${VALIDATION}\n        let offset`, `${QUERY}\n        let offset`);
    }),
  );
  assert.equal(result.status, 1);
  assert.match(
    result.stderr,
    /queries\.rs must validate the Storefront search before constructing owner SQL/,
  );
});

test("rejects an owner file that lost the Storefront search validation", () => {
  const result = rejects((root) =>
    rewrite(root, ownerPath, (source) => {
      const entry = source.indexOf("pub async fn list_published_products_with_query(");
      assert.ok(entry >= 0, "the Storefront search entry point must exist");
      const head = source.slice(0, entry);
      const body = source.slice(entry).replace(`${VALIDATION}\n`, "");
      return `${head}${body}`;
    }),
  );
  assert.equal(result.status, 1);
  assert.match(
    result.stderr,
    /queries\.rs is missing types::validate_storefront_product_search|queries\.rs must validate the Storefront search before constructing owner SQL/,
  );
});

test("rejects truncating the search value instead of rejecting it", () => {
  const result = rejects((root) =>
    rewrite(root, typesPath, (source) =>
      source.replace(
        `pub(crate) fn validate_storefront_product_search(search: Option<&str>) -> CommerceResult<()> {
    let Some(search) = search.map(str::trim).filter(|search| !search.is_empty()) else {
        return Ok(());
    };`,
        `pub(crate) fn validate_storefront_product_search(search: Option<&str>) -> CommerceResult<()> {
    let Some(search) = search.map(str::trim).filter(|search| !search.is_empty()) else {
        return Ok(());
    };
    let search = search.chars().take(MAX_STOREFRONT_PRODUCT_SEARCH_BYTES).collect::<String>();
    let search = search.as_str();`,
      ),
    ),
  );
  assert.equal(result.status, 1);
  assert.match(
    result.stderr,
    /types\.rs must reject over-bound Storefront search instead of truncating it/,
  );
});

test("rejects a search bound that no longer fits the Index TextLike pattern", () => {
  const result = rejects((root) =>
    rewrite(root, typesPath, (source) =>
      source.replace(
        "pub const MAX_STOREFRONT_PRODUCT_SEARCH_BYTES: usize = 1022;",
        "pub const MAX_STOREFRONT_PRODUCT_SEARCH_BYTES: usize = 1023;",
      ),
    ),
  );
  assert.equal(result.status, 1);
  assert.match(
    result.stderr,
    /is missing pub const MAX_STOREFRONT_PRODUCT_SEARCH_BYTES: usize = 1022;/,
  );
});

test("rejects a second search bound owned by the Index shadow", () => {
  const result = rejects((root) =>
    rewrite(root, shadowPath, (source) =>
      source.replace(
        "services::MAX_STOREFRONT_PRODUCT_SEARCH_BYTES",
        "const MAX_TEXT_LIKE_PATTERN_BYTES: usize = 1024;\n    services::MAX_STOREFRONT_PRODUCT_SEARCH_BYTES",
      ),
    ),
  );
  assert.equal(result.status, 1);
  assert.match(result.stderr, /must consume the Product-owned search bound/);
});
