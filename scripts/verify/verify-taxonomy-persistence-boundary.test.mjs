#!/usr/bin/env node

import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const verifier = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  'verify-taxonomy-persistence-boundary.mjs',
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
    env: { ...process.env, RUSTOK_TAXONOMY_BOUNDARY_ROOT: root },
    encoding: 'utf8',
  });
}

function expectFailure(root, pathPattern, tokenPattern, message) {
  const result = run(root);
  assert.notEqual(result.status, 0, message);
  assert.match(result.stderr, pathPattern);
  if (tokenPattern) assert.match(result.stderr, tokenPattern);
}

const root = fs.mkdtempSync(path.join(os.tmpdir(), 'rustok-taxonomy-boundary-'));
try {
  write(
    root,
    'crates/rustok-taxonomy/src/services.rs',
    'fn owner() { let _ = taxonomy_term::Entity; }\n',
  );
  write(
    root,
    'crates/rustok-product/src/migrations/m0001.rs',
    'fn migration() { let _ = taxonomy_term::Entity; }\n',
  );
  write(
    root,
    'crates/rustok-product/src/entities/product_tag.rs',
    'fn relation() { let _ = taxonomy_term::Entity; }\n',
  );
  write(
    root,
    'crates/rustok-blog/tests/taxonomy_tags.rs',
    'fn storage_assertion() { let _ = taxonomy_term_translation::Entity; }\n',
  );
  write(
    root,
    'crates/rustok-search/src/blog_projector.rs',
    [
      'const SQL: &str = r#"JOIN taxonomy_terms term; taxonomy_term::Entity"#;',
      '// taxonomy_term_translation::Entity is documentation only.',
      'fn lifetime_passthrough<\'a>(value: &\'a str) -> &\'a str { value }',
      '',
    ].join('\n'),
  );
  write(
    root,
    'crates/rustok-content-orchestration/src/lib.rs',
    [
      'fn production_before_tests() {}',
      '#[cfg(all(test, feature = "fixture"))]',
      'mod tests {',
      '    fn storage_assertion() {',
      '        let fake = "}";',
      '        let _ = taxonomy_term::Entity;',
      '    }',
      '}',
      'fn production_after_tests() {}',
      '',
    ].join('\n'),
  );

  let result = run(root);
  assert.equal(
    result.status,
    0,
    `allowed fixture should pass:\nstdout=${result.stdout}\nstderr=${result.stderr}`,
  );

  const blogViolation = 'crates/rustok-blog/src/services/tag.rs';
  write(
    root,
    blogViolation,
    [
      'use rustok_taxonomy::{TaxonomyService, entities::{taxonomy_term}};',
      'fn bypass() { let _ = taxonomy_term::Entity; }',
      '',
    ].join('\n'),
  );
  expectFailure(
    root,
    /crates\/rustok-blog\/src\/services\/tag\.rs/,
    /Taxonomy persistence/,
    'runtime persistence bypass must fail',
  );
  remove(root, blogViolation);

  const forumViolation = 'crates/rustok-forum/src/services/tag.rs';
  write(
    root,
    forumViolation,
    'use rustok_taxonomy::entities as taxonomy_entities;\nfn bypass() {}\n',
  );
  expectFailure(
    root,
    /rustok-forum\/src\/services\/tag\.rs/,
    /rustok_taxonomy::entities/,
    'direct entities-module import must fail',
  );
  remove(root, forumViolation);

  const mixedFile = 'crates/rustok-content-orchestration/src/lib.rs';
  write(
    root,
    mixedFile,
    [
      '#[cfg(test)]',
      'mod tests {',
      '    fn storage_assertion() { let _ = taxonomy_term::Entity; }',
      '}',
      'fn production_after_tests() { let _ = taxonomy_term_route_key::Entity; }',
      '',
    ].join('\n'),
  );
  expectFailure(
    root,
    /rustok-content-orchestration\/src\/lib\.rs/,
    /taxonomy_term_route_key::Entity/,
    'production persistence bypass after a test-only module must still fail',
  );

  write(
    root,
    mixedFile,
    [
      '#[cfg(not(test))]',
      'fn production_when_not_testing() { let _ = taxonomy_term_alias::Entity; }',
      '',
    ].join('\n'),
  );
  expectFailure(
    root,
    /rustok-content-orchestration\/src\/lib\.rs/,
    /taxonomy_term_alias::Entity/,
    'cfg(not(test)) is production and must not be masked',
  );

  write(
    root,
    mixedFile,
    [
      '#[cfg(any(test, feature = "runtime"))]',
      'fn runtime_feature_path() { let _ = taxonomy_term_translation::Entity; }',
      '',
    ].join('\n'),
  );
  expectFailure(
    root,
    /rustok-content-orchestration\/src\/lib\.rs/,
    /taxonomy_term_translation::Entity/,
    'cfg(any(test, runtime feature)) may compile in production and must not be masked',
  );

  console.log('[verify-taxonomy-persistence-boundary.test] PASS');
} finally {
  fs.rmSync(root, { recursive: true, force: true });
}
