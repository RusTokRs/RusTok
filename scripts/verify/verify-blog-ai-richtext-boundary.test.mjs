#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const scriptPath = path.resolve('scripts/verify/verify-blog-ai-richtext-boundary.mjs');

function writeFixtureFile(root, relativePath, content) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-ai-richtext-'));
  const shimExports = options.exposeMigrations
    ? 'CreatePostInput, PostResponse, PostService, UpdatePostInput, migrations, richtext'
    : 'CreatePostInput, PostResponse, PostService, UpdatePostInput, richtext';
  writeFixtureFile(
    root,
    'crates/rustok-ai/src/rustok_blog.rs',
    `pub use rustok_blog_owner::{\n    ${shimExports},\n};\n${options.testOnlyMigrations ? '#[cfg(test)]\npub use rustok_blog_owner::migrations;\n' : ''}`,
  );

  const direct = [
    'use crate::rustok_blog::{CreatePostInput, PostService, UpdatePostInput};',
    'impl DirectTaskHandler for BlogDraftHandler {',
    'let service = PostService::new(runtime.db_clone(), runtime.event_bus());',
    'crate::rustok_blog::richtext::article_document_from_plain_text(&body)',
    'crate::rustok_blog::richtext::article_document_from_plain_text(body)',
    'existing_post.map(|post| post.content_plain_text.clone())',
    'blog_draft_must_remain_unpublished()',
    options.publishDraft ? 'publish: true' : 'publish: false',
    options.markdownWriter ? 'CONTENT_FORMAT_MARKDOWN' : '',
    '}',
    options.testOnlyMigrations ? '#[cfg(test)]\nmod tests { fn fixture() { crate::rustok_blog::migrations::migrations(); } }' : '',
  ].join('\n');
  writeFixtureFile(root, 'crates/rustok-ai/src/direct.rs', direct);

  const evidence = {
    schema_version: 1,
    module: 'blog',
    surface: 'ai_blog_draft_richtext_boundary',
    status: 'source_verified_no_compile',
    compile_policy: 'not_run_by_request',
    shim: {
      path: 'crates/rustok-ai/src/rustok_blog.rs',
      owner: 'rustok_blog_owner',
      allowed_reexports: options.evidenceAllowsMigrations
        ? ['CreatePostInput', 'PostResponse', 'PostService', 'UpdatePostInput', 'migrations', 'richtext']
        : ['CreatePostInput', 'PostResponse', 'PostService', 'UpdatePostInput', 'richtext'],
      forbidden_reexports: ['migrations', 'entities', 'graphql', 'http', 'seo_targets'],
    },
    writer: {
      path: 'crates/rustok-ai/src/direct.rs',
      required_markers: [
        'use crate::rustok_blog::{CreatePostInput, PostService, UpdatePostInput};',
        'impl DirectTaskHandler for BlogDraftHandler',
        'PostService::new(runtime.db_clone(), runtime.event_bus())',
        'crate::rustok_blog::richtext::article_document_from_plain_text(&body)',
        'crate::rustok_blog::richtext::article_document_from_plain_text(body)',
        'existing_post.map(|post| post.content_plain_text.clone())',
        'blog_draft_must_remain_unpublished()',
        'publish: false',
      ],
      forbidden_markers: [
        'crate::rustok_blog::migrations',
        'CONTENT_FORMAT_MARKDOWN',
        'body_format:',
        'content_json:',
      ],
    },
    guardrail: 'scripts/verify/verify-blog-ai-richtext-boundary.mjs',
    guardrail_test: 'scripts/verify/verify-blog-ai-richtext-boundary.test.mjs',
  };
  writeFixtureFile(
    root,
    'crates/rustok-blog/contracts/evidence/blog-ai-richtext-boundary.json',
    JSON.stringify(evidence),
  );
  writeFixtureFile(
    root,
    'crates/rustok-blog/docs/implementation-plan.md',
    [
      '# plan',
      'AI Blog owner shim',
      'crates/rustok-blog/contracts/evidence/blog-ai-richtext-boundary.json',
      'scripts/verify/verify-blog-ai-richtext-boundary.mjs',
      'scripts/verify/verify-blog-ai-richtext-boundary.test.mjs',
      '35. Bound the AI Blog owner shim.',
    ].join('\n'),
  );
  writeFixtureFile(
    root,
    'crates/rustok-blog/contracts/blog-fba-registry.json',
    JSON.stringify({
      schema_version: 13,
      verification_chain: {
        source_gates: {
          ai_richtext_boundary: {
            package_script: 'verify:blog:ai-richtext-boundary',
            test_package_script: 'test:verify:blog:ai-richtext-boundary',
            verifier: 'scripts/verify/verify-blog-ai-richtext-boundary.mjs',
            self_test: 'scripts/verify/verify-blog-ai-richtext-boundary.test.mjs',
            evidence: 'crates/rustok-blog/contracts/evidence/blog-ai-richtext-boundary.json',
          },
        },
      },
    }),
  );
  writeFixtureFile(
    root,
    'scripts/verify/verify-blog-ai-richtext-boundary.test.mjs',
    'fixture marker',
  );
  writeFixtureFile(
    root,
    'package.json',
    JSON.stringify({
      scripts: {
        'verify:blog:ai-richtext-boundary':
          'node scripts/verify/verify-blog-ai-richtext-boundary.mjs',
        'test:verify:blog:ai-richtext-boundary':
          'node scripts/verify/verify-blog-ai-richtext-boundary.test.mjs',
        'verify:blog:fba':
          'node scripts/verify/verify-blog-fba.mjs && npm run verify:blog:ai-richtext-boundary',
        'test:verify:blog:fba': options.omitTestAggregate
          ? 'node scripts/verify/verify-blog-fba.test.mjs'
          : 'node scripts/verify/verify-blog-fba.test.mjs && npm run test:verify:blog:ai-richtext-boundary',
      },
    }),
  );
  return root;
}

function run(root) {
  return spawnSync(process.execPath, [scriptPath], {
    cwd: root,
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: 'utf8',
  });
}

test('Blog AI richtext verifier accepts the canonical owner-only draft boundary', () => {
  const result = run(fixture());
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /canonical owner-only richtext boundary/);
});

test('Blog AI richtext verifier rejects migration re-exports through the AI shim', () => {
  const result = run(fixture({ exposeMigrations: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /forbidden owner re-export migrations|owner re-export drift/);
});

test('Blog AI richtext verifier permits test-only migration fixtures', () => {
  const result = run(fixture({ testOnlyMigrations: true }));
  assert.equal(result.status, 0, result.stderr);
});

test('Blog AI richtext verifier rejects evidence that permits migration re-exports', () => {
  const result = run(fixture({ exposeMigrations: true, evidenceAllowsMigrations: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /forbidden owner re-export migrations/);
});

test('Blog AI richtext verifier rejects Markdown-shaped draft writers', () => {
  const result = run(fixture({ markdownWriter: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /forbidden CONTENT_FORMAT_MARKDOWN/);
});

test('Blog AI richtext verifier rejects publish-on-create drift', () => {
  const result = run(fixture({ publishDraft: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /missing publish: false/);
});

test('Blog AI richtext verifier rejects missing aggregate self-test wiring', () => {
  const result = run(fixture({ omitTestAggregate: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /self-test aggregate does not include AI richtext boundary fixture/);
});
