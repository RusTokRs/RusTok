#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const verifier = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  "verify-taxonomy-ownership-boundary.mjs",
);

function write(root, relativePath, content) {
  const target = path.join(root, relativePath);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, content);
}

function remove(root, relativePath) {
  fs.rmSync(path.join(root, relativePath), { force: true });
}

function run(root) {
  return spawnSync(process.execPath, [verifier], {
    cwd: root,
    env: {
      ...process.env,
      RUSTOK_TAXONOMY_OWNERSHIP_ROOT: root,
    },
    encoding: "utf8",
  });
}

function expectSuccess(root, message) {
  const result = run(root);
  assert.equal(
    result.status,
    0,
    `${message}:\nstdout=${result.stdout}\nstderr=${result.stderr}`,
  );
  assert.match(result.stdout, /Taxonomy ownership boundary checks passed/);
}

function expectFailure(root, pathPattern, tokenPattern, message) {
  const result = run(root);
  assert.notEqual(result.status, 0, message);
  assert.match(result.stderr, /Taxonomy ownership boundary verification failed/);
  assert.match(result.stderr, pathPattern);
  if (tokenPattern) assert.match(result.stderr, tokenPattern);
}

function writeBaseline(root) {
  write(root, "crates/rustok-taxonomy/src/lib.rs", "pub fn flat_vocabulary() {}\n");

  write(
    root,
    "crates/rustok-blog/src/migrations/m20260328_000002_create_blog_taxonomy_tables.rs",
    [
      "fn migration() {",
      "    let _ = BlogCategories::ParentId;",
      "    let _ = TaxonomyTerms::Id;",
      "}",
      "// .table(BlogPostTags::Table)",
      "// .table(BlogCategoryTranslations::Table)",
      "// .to(TaxonomyTerms::Table, TaxonomyTerms::Id)",
      "",
    ].join("\n"),
  );
  write(
    root,
    "crates/rustok-blog/src/entities/blog_post_tag.rs",
    '#[sea_orm(table_name = "blog_post_tags")]\npub struct Model;\n',
  );

  write(
    root,
    "crates/rustok-forum/src/migrations/m20260328_000001_create_forum_tables.rs",
    [
      "fn migration() {",
      "    let _ = ForumCategories::ParentId;",
      "    let _ = ForumCategoryTranslations::Locale;",
      "}",
      "// .table(ForumCategories::Table)",
      "// .table(ForumCategoryTranslations::Table)",
      "",
    ].join("\n"),
  );
  write(
    root,
    "crates/rustok-forum/src/migrations/m20260329_000005_create_forum_topic_tags.rs",
    [
      "fn migration() {",
      "    let _ = ForumTopicTags::TopicId;",
      "    let _ = ForumTopicTags::TermId;",
      "}",
      "// .table(ForumTopicTags::Table)",
      "// .to(TaxonomyTerms::Table, TaxonomyTerms::Id)",
      "",
    ].join("\n"),
  );
  write(
    root,
    "crates/rustok-forum/src/entities/forum_topic_tag.rs",
    '#[sea_orm(table_name = "forum_topic_tags")]\npub struct Model;\n',
  );

  write(
    root,
    "crates/rustok-product/src/migrations/m20260329_000001_create_product_tags.rs",
    [
      "fn migration() {",
      "    let _ = ProductTags::ProductId;",
      "    let _ = ProductTags::TermId;",
      "}",
      "// .table(ProductTags::Table)",
      "// .to(TaxonomyTerms::Table, TaxonomyTerms::Id)",
      "",
    ].join("\n"),
  );
  write(
    root,
    "crates/rustok-product/src/entities/product_tag.rs",
    '#[sea_orm(table_name = "product_tags")]\npub struct Model;\n',
  );
  write(
    root,
    "crates/rustok-product/src/migrations/m20260701_000001_create_product_catalog_attributes.rs",
    [
      'const SQL: &str = r#"',
      "CREATE TABLE IF NOT EXISTS catalog_categories (",
      "    id UUID PRIMARY KEY,",
      "    parent_id UUID REFERENCES catalog_categories(id)",
      ");",
      "CREATE TABLE IF NOT EXISTS catalog_category_closure (depth INTEGER NOT NULL);",
      "CREATE TABLE IF NOT EXISTS product_categories (product_id UUID NOT NULL);",
      '"#;',
      "",
    ].join("\n"),
  );
  write(
    root,
    "crates/rustok-product/docs/category-locale-contract.md",
    [
      "Product catalog categories remain a Product-owned tree/closure aggregate.",
      "Product owns category parent/child relations, closure rows, moves, deletion",
      "Taxonomy owns shared vocabulary identities and localized taxonomy route keys",
      "",
    ].join("\n"),
  );

  write(
    root,
    "crates/rustok-profiles/src/migrations/m20260330_000002_create_profile_tags.rs",
    [
      "fn migration() {",
      "    let _ = ProfileTags::ProfileUserId;",
      "    let _ = ProfileTags::TermId;",
      "}",
      "// .table(ProfileTags::Table)",
      "// .to(TaxonomyTerms::Table, TaxonomyTerms::Id)",
      "",
    ].join("\n"),
  );
  write(
    root,
    "crates/rustok-profiles/src/entities/profile_tag.rs",
    '#[sea_orm(table_name = "profile_tags")]\npub struct Model;\n',
  );
}

const root = fs.mkdtempSync(path.join(os.tmpdir(), "rustok-taxonomy-ownership-"));
try {
  writeBaseline(root);
  expectSuccess(root, "baseline owner-module fixture should pass");

  write(
    root,
    "crates/rustok-taxonomy/src/hierarchy.rs",
    "pub struct InvalidHierarchy { pub parent_id: i64 }\n",
  );
  expectFailure(
    root,
    /crates\/rustok-taxonomy\/src\/hierarchy\.rs/,
    /generic parent_id/,
    "Taxonomy parent_id must fail closed",
  );
  remove(root, "crates/rustok-taxonomy/src/hierarchy.rs");

  write(
    root,
    "crates/rustok-taxonomy/src/migrations/m0001_consumer_relation.rs",
    'const TABLE: &str = "blog_post_tags";\n',
  );
  expectFailure(
    root,
    /m0001_consumer_relation\.rs/,
    /consumer attachment storage/,
    "Taxonomy-owned consumer relation storage must fail closed",
  );
  remove(root, "crates/rustok-taxonomy/src/migrations/m0001_consumer_relation.rs");

  write(
    root,
    "crates/rustok-taxonomy/src/entities/generic_attachment.rs",
    "pub struct GenericAttachment { pub owner_type: String, pub owner_id: i64 }\n",
  );
  expectFailure(
    root,
    /generic_attachment\.rs/,
    /polymorphic owner_type\/owner_id/,
    "generic polymorphic Taxonomy attachment storage must fail closed",
  );
  remove(root, "crates/rustok-taxonomy/src/entities/generic_attachment.rs");

  write(
    root,
    "crates/rustok-forum/src/migrations/m20260328_000001_create_forum_tables.rs",
    [
      "fn migration() { let _ = ForumCategories::ParentId; }",
      "// .table(ForumCategories::Table)",
      "",
    ].join("\n"),
  );
  expectFailure(
    root,
    /m20260328_000001_create_forum_tables\.rs/,
    /ForumCategoryTranslations::Table/,
    "Forum category translation ownership drift must fail closed",
  );
  writeBaseline(root);

  write(
    root,
    "crates/rustok-product/src/migrations/m20260701_000001_create_product_catalog_attributes.rs",
    [
      'const SQL: &str = r#"',
      "CREATE TABLE IF NOT EXISTS catalog_categories (",
      "    parent_id UUID REFERENCES catalog_categories(id)",
      ");",
      "CREATE TABLE IF NOT EXISTS product_categories (product_id UUID NOT NULL);",
      '"#;',
      "",
    ].join("\n"),
  );
  expectFailure(
    root,
    /m20260701_000001_create_product_catalog_attributes\.rs/,
    /catalog_category_closure/,
    "Product category closure ownership drift must fail closed",
  );
  writeBaseline(root);

  remove(root, "crates/rustok-profiles/src/entities/profile_tag.rs");
  expectFailure(
    root,
    /crates\/rustok-profiles\/src\/entities\/profile_tag\.rs/,
    /missing owner-owned Taxonomy consumer artifact/,
    "missing owner relation artifact must fail closed",
  );

  console.log("[verify-taxonomy-ownership-boundary-self-test] PASS");
} finally {
  fs.rmSync(root, { recursive: true, force: true });
}
