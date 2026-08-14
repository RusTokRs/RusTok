#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const verifier = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  "verify-taxonomy-contract-matrix.mjs",
);

function write(root, relative, content) {
  const target = path.join(root, relative);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, content);
}

function run(root) {
  return spawnSync(process.execPath, [verifier], {
    cwd: root,
    env: {
      ...process.env,
      RUSTOK_TAXONOMY_CONTRACT_ROOT: root,
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
  assert.match(result.stdout, /Taxonomy contract matrix checks passed/);
}

function expectFailure(root, pattern, message) {
  const result = run(root);
  assert.notEqual(result.status, 0, message);
  assert.match(result.stderr, /Taxonomy contract matrix verification failed/);
  assert.match(result.stderr, pattern);
}

const taxonomyDependency = '[dependencies]\ntaxonomy = { version_req = ">=0.1.0" }\n';

function writeBaseline(root) {
  write(
    root,
    "crates/rustok-blog/rustok-module.toml",
    `[module]\nslug = "blog"\n${taxonomyDependency}`,
  );
  write(
    root,
    "crates/rustok-blog/CRATE_API.md",
    [
      "Canonical tag identity now lives in shared `rustok-taxonomy`",
      "rustok-blog keeps `blog_post_tags` as the module-owned relation table",
      "",
    ].join("\n"),
  );

  write(
    root,
    "crates/rustok-forum/rustok-module.toml",
    `[module]\nslug = "forum"\n${taxonomyDependency}`,
  );
  write(
    root,
    "crates/rustok-forum/docs/README.md",
    [
      "tag attachments via `forum_topic_tags` with shared vocabulary in `rustok-taxonomy`",
      "uses `rustok-taxonomy` as a shared dictionary for tag identity",
      "",
    ].join("\n"),
  );

  write(
    root,
    "crates/rustok-product/rustok-module.toml",
    `[module]\nslug = "product"\n${taxonomyDependency}`,
  );
  write(
    root,
    "crates/rustok-product/README.md",
    [
      "Product-owned relation storage for taxonomy-backed tags (`product_tags`).",
      "Depends on `rustok-taxonomy` for shared scope-aware tag dictionary",
      "",
    ].join("\n"),
  );

  write(
    root,
    "crates/rustok-profiles/rustok-module.toml",
    `[module]\nslug = "profiles"\n${taxonomyDependency}`,
  );
  write(
    root,
    "crates/rustok-profiles/README.md",
    [
      "Own profile-to-taxonomy relation storage via `profile_tags`.",
      "Depends on `rustok-taxonomy` for shared scope-aware tags while keeping `profile_tags` module-owned.",
      "",
    ].join("\n"),
  );

  write(
    root,
    "crates/rustok-taxonomy/src/dto.rs",
    [
      "pub enum TaxonomyTermKind {",
      '    #[sea_orm(string_value = "tag")]',
      "    Tag,",
      "}",
      "",
    ].join("\n"),
  );
  write(
    root,
    "crates/rustok-taxonomy/tests/localized_route_lookup.rs",
    [
      "public_route_lookup_uses_registry_authority_over_unregistered_legacy_alias",
      "owner_batch_collapses_equivalent_labels_and_normalizes_scope_and_locale",
      "owner_batch_prefers_module_term_before_global_across_locale_fallback",
      "owner_batch_reuses_global_term_when_module_term_is_absent",
      "owner_batch_prefers_module_canonical_key_before_global_route",
      "owner_batch_reuses_global_canonical_key_without_shadow_module_term",
      "owner_batch_canonical_key_lookup_is_tenant_isolated",
      "",
    ].join("\n"),
  );
  write(
    root,
    "crates/rustok-taxonomy/tests/route_key_registry.rs",
    [
      "hard_delete_removes_lookup_and_allows_route_identity_reuse",
      "database_primary_key_rejects_second_route_owner",
      "",
    ].join("\n"),
  );
  write(
    root,
    ".github/workflows/taxonomy-lookup-contract.yml",
    [
      '- "crates/rustok-taxonomy/src/dto.rs"',
      "cargo test --locked -p rustok-taxonomy --test localized_route_lookup --test route_key_registry",
      "",
    ].join("\n"),
  );
  write(
    root,
    ".github/workflows/taxonomy-ownership-boundary.yml",
    [
      '- "crates/rustok-blog/rustok-module.toml"',
      '- "crates/rustok-blog/CRATE_API.md"',
      '- "crates/rustok-forum/rustok-module.toml"',
      '- "crates/rustok-forum/docs/README.md"',
      '- "crates/rustok-product/rustok-module.toml"',
      '- "crates/rustok-product/README.md"',
      '- "crates/rustok-profiles/rustok-module.toml"',
      '- "crates/rustok-profiles/README.md"',
      '- "scripts/verify/verify-taxonomy-contract-matrix.mjs"',
      '- "scripts/verify/verify-taxonomy-contract-matrix.test.mjs"',
      "run: node scripts/verify/verify-taxonomy-contract-matrix.test.mjs",
      "run: node scripts/verify/verify-taxonomy-contract-matrix.mjs",
      "",
    ].join("\n"),
  );
}

const root = fs.mkdtempSync(path.join(os.tmpdir(), "rustok-taxonomy-contract-"));
try {
  writeBaseline(root);
  expectSuccess(root, "baseline contract matrix should pass");

  write(
    root,
    "crates/rustok-product/rustok-module.toml",
    '[module]\nslug = "product"\n[dependencies]\noutbox = { version_req = ">=0.1.0" }\n',
  );
  expectFailure(
    root,
    /Product must declare taxonomy >=0\.1\.0/,
    "consumer manifest drift must fail closed",
  );
  writeBaseline(root);

  write(
    root,
    "crates/rustok-forum/docs/README.md",
    "uses `rustok-taxonomy` as a shared dictionary for tag identity\n",
  );
  expectFailure(
    root,
    /forum_topic_tags/,
    "owner public relation contract drift must fail closed",
  );
  writeBaseline(root);

  write(
    root,
    "crates/rustok-taxonomy/src/dto.rs",
    [
      "pub enum TaxonomyTermKind {",
      "    Tag,",
      "    Category,",
      "}",
      "",
    ].join("\n"),
  );
  expectFailure(
    root,
    /demonstrated kind baseline must remain exactly Tag/,
    "speculative kind expansion must fail closed",
  );
  writeBaseline(root);

  write(
    root,
    ".github/workflows/taxonomy-lookup-contract.yml",
    "cargo test --locked -p rustok-taxonomy --test localized_route_lookup --test route_key_registry\n",
  );
  expectFailure(
    root,
    /crates\/rustok-taxonomy\/src\/dto\.rs/,
    "kind changes must stay inside the focused lookup workflow trigger set",
  );
  writeBaseline(root);

  write(
    root,
    ".github/workflows/taxonomy-ownership-boundary.yml",
    [
      '- "crates/rustok-blog/CRATE_API.md"',
      '- "crates/rustok-forum/rustok-module.toml"',
      '- "crates/rustok-forum/docs/README.md"',
      '- "crates/rustok-product/rustok-module.toml"',
      '- "crates/rustok-product/README.md"',
      '- "crates/rustok-profiles/rustok-module.toml"',
      '- "crates/rustok-profiles/README.md"',
      '- "scripts/verify/verify-taxonomy-contract-matrix.mjs"',
      '- "scripts/verify/verify-taxonomy-contract-matrix.test.mjs"',
      "run: node scripts/verify/verify-taxonomy-contract-matrix.test.mjs",
      "run: node scripts/verify/verify-taxonomy-contract-matrix.mjs",
      "",
    ].join("\n"),
  );
  expectFailure(
    root,
    /crates\/rustok-blog\/rustok-module\.toml/,
    "consumer manifest changes must stay inside the focused ownership workflow trigger set",
  );

  console.log("[verify-taxonomy-contract-matrix.test] PASS");
} finally {
  fs.rmSync(root, { recursive: true, force: true });
}
