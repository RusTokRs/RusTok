#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const scriptPath = path.resolve('scripts/verify/verify-blog-richtext-offline-backfill.mjs');
const sourcePath = 'crates/rustok-blog/src/bin/blog_article_richtext_backfill.rs';
const evidencePath = 'crates/rustok-blog/contracts/evidence/blog-richtext-offline-backfill.json';

function writeFixtureFile(root, relativePath, content) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function sourceText(options = {}) {
  return `const TARGET_FORMAT: &str = "richtext";
async fn preflight_pass(
async fn apply_pass(
async fn optimistic_update(
LEFT JOIN blog_posts
struct Cursor
updated_at = $6
repair the owner relation before backfill
--apply
--allow-markdown-plain-text
article_document_from_plain_text
normalize_article
canonical_article_body
full successful preflight
${options.missingOptimistic ? '' : 'optimistic update conflict'}
post-apply verification failed
ReportRecord
body_format = $2
body_format = ?
${options.checkpoint ? 'persist_checkpoint' : ''}`;
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-richtext-backfill-'));
  writeFixtureFile(root, sourcePath, sourceText(options));
  writeFixtureFile(root, evidencePath, JSON.stringify({
    schema_version: 1,
    module: 'blog',
    surface: 'article_richtext_offline_backfill',
    status: 'executable_no_run',
    compile_policy: 'not_run_by_request',
    runner: 'cargo run -p rustok-blog --bin blog_article_richtext_backfill --',
    source: sourcePath,
    verifier: 'scripts/verify/verify-blog-richtext-offline-backfill.mjs',
    safety: {
      default_mode: options.unsafeDefault ? 'apply' : 'dry_run',
      apply_flag: '--apply',
      markdown_plain_text_flag: '--allow-markdown-plain-text',
      preflight_before_apply: true,
      optimistic_updates: true,
      orphan_rows_fail_closed: true,
      stable_cursor: 'updated_at_id',
      optimistic_updated_at_predicate: true,
      checkpoint_mutation: false,
    },
  }));
  writeFixtureFile(root, 'crates/rustok-blog/contracts/evidence/blog-richtext-cutover-inventory.json',
    JSON.stringify({
      checks: [{ name: 'offline_backfill', status: 'executable_no_run', path: sourcePath }],
      completion_conditions: ['legacy_rows_have_owner_specific_dry_run_backfill'],
    }));
  writeFixtureFile(root, 'crates/rustok-blog/docs/implementation-plan.md',
    `${sourcePath} ${evidencePath} --allow-markdown-plain-text offline backfill`);
  writeFixtureFile(root, 'crates/rustok-blog/docs/richtext-cutover-inventory.md',
    `${sourcePath} ${evidencePath} Dry-run is the default optimistic`);
  writeFixtureFile(root, 'scripts/verify/verify-blog-richtext-offline-backfill.test.mjs', 'fixture marker');
  writeFixtureFile(root, 'package.json', JSON.stringify({
    scripts: {
      'verify:blog:richtext-offline-backfill': 'node scripts/verify/verify-blog-richtext-offline-backfill.mjs',
      'test:verify:blog:richtext-offline-backfill': 'node scripts/verify/verify-blog-richtext-offline-backfill.test.mjs',
      'verify:blog:fba': 'node scripts/verify/verify-blog-fba.mjs && npm run verify:blog:richtext-offline-backfill',
      'test:verify:blog:fba': options.omitTestAggregate
        ? 'node scripts/verify/verify-blog-fba.test.mjs'
        : 'node scripts/verify/verify-blog-fba.test.mjs && npm run test:verify:blog:richtext-offline-backfill',
    },
  }));
  return root;
}

function run(root) {
  return spawnSync(process.execPath, [scriptPath], { cwd: root, encoding: 'utf8' });
}

test('Blog richtext offline backfill verifier accepts the canonical dry-run contract', () => {
  const result = run(fixture());
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /dry-run\/apply safety contract is consistent/);
});

test('Blog richtext offline backfill verifier rejects apply-by-default evidence', () => {
  const result = run(fixture({ unsafeDefault: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /safety contract drift/);
});

test('Blog richtext offline backfill verifier rejects a missing optimistic conflict guard', () => {
  const result = run(fixture({ missingOptimistic: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /backfill source missing optimistic update conflict/);
});

test('Blog richtext offline backfill verifier rejects checkpoint mutation', () => {
  const result = run(fixture({ checkpoint: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /backfill source contains forbidden persist_checkpoint/);
});

test('Blog richtext offline backfill verifier rejects missing aggregate self-test wiring', () => {
  const result = run(fixture({ omitTestAggregate: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /self-test aggregate does not include offline backfill fixture/);
});
