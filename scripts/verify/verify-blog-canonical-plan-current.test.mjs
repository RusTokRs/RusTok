#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const verifier = path.join(repositoryRoot, 'scripts/verify/verify-blog-canonical-plan-current.mjs');
const files = [
  'crates/rustok-blog/contracts/evidence/blog-canonical-plan-current-source.json',
  'crates/rustok-blog/docs/implementation-plan-current.md',
  'crates/rustok-blog/docs/implementation-plan.md',
  'crates/rustok-blog/docs/implementation-plan-slice-97.md',
  'crates/rustok-blog/docs/implementation-plan-slice-98.md',
  'crates/rustok-blog/docs/implementation-plan-slice-99.md',
  'crates/rustok-blog/docs/implementation-plan-slice-100.md',
  'crates/rustok-blog/docs/implementation-plan-slice-101.md',
  'crates/rustok-blog/docs/README.md',
  'crates/rustok-blog/contracts/evidence/blog-comments-tcp-transport.json',
  'crates/rustok-blog/contracts/evidence/blog-comments-tcp-server-adapter.json',
  'crates/rustok-blog/contracts/evidence/blog-comments-tcp-listener-lifecycle.json',
  'crates/rustok-blog/contracts/evidence/blog-category-translation-postgres-source.json',
  'crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json',
  'crates/rustok-blog/contracts/evidence/blog-comments-storefront-write-surface.json',
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
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-current-plan-'));
  for (const relativePath of files) {
    const source = path.join(repositoryRoot, relativePath);
    const target = absolute(root, relativePath);
    mkdirSync(path.dirname(target), { recursive: true });
    cpSync(source, target);
  }
  mutator(root);
  return root;
}

function run(root) {
  return spawnSync(process.execPath, [verifier], {
    cwd: repositoryRoot,
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: 'utf8',
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

function mutateJson(root, relativePath, mutator) {
  const target = absolute(root, relativePath);
  const value = JSON.parse(readFileSync(target, 'utf8'));
  mutator(value);
  write(root, relativePath, `${JSON.stringify(value, null, 2)}\n`);
}

test('accepts the canonical Blog current implementation cursor', () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /cursor=slice-101/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects reopening remote transport as live current-cursor work', () => {
  const result = rejects((root) => {
    const relativePath = 'crates/rustok-blog/docs/implementation-plan-current.md';
    const source = readFileSync(absolute(root, relativePath), 'utf8');
    write(
      root,
      relativePath,
      source.replace(
        '`remote_comments_transport = source_implemented_maintainer_execution_pending`',
        '`remote_comments_transport = remote transport remains pending`',
      ),
    );
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /remote transport|current/);
});

test('rejects promoting a new autonomous source gap without a fresh audit', () => {
  const result = rejects((root) => {
    mutateJson(
      root,
      'crates/rustok-blog/contracts/evidence/blog-canonical-plan-current-source.json',
      (value) => {
        value.planning_result.independent_production_source_gap_identified = true;
      },
    );
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /planning\/execution claim drift/);
});

test('rejects Translation PostgreSQL execution promotion without retained execution', () => {
  const result = rejects((root) => {
    mutateJson(
      root,
      'crates/rustok-blog/contracts/evidence/blog-category-translation-postgres-source.json',
      (value) => {
        value.source_contract.postgres_execution_observed = true;
      },
    );
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Translation PostgreSQL source status drift/);
});

test('rejects reviving an active storefront comment form', () => {
  const result = rejects((root) => {
    mutateJson(
      root,
      'crates/rustok-blog/contracts/evidence/blog-comments-storefront-write-surface.json',
      (value) => {
        value.source_contract.comment_form_present = true;
        value.source_contract.create_comment_surface_present = true;
      },
    );
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /write-surface evidence drift/);
});

test('rejects a cached snapshot regression back to planned source work', () => {
  const result = rejects((root) => {
    mutateJson(
      root,
      'crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json',
      (value) => {
        value.storefront_read_degradation.cached_thread_snapshot = 'planned';
      },
    );
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /storefront fallback current-state drift/);
});

test('rejects listing the historical plan before the canonical current cursor', () => {
  const result = rejects((root) => {
    const relativePath = 'crates/rustok-blog/docs/README.md';
    const source = readFileSync(absolute(root, relativePath), 'utf8');
    const current = '[Current Implementation Cursor](./implementation-plan-current.md)';
    const historical = '[Historical Implementation Plan](./implementation-plan.md)';
    write(
      root,
      relativePath,
      source.replace(current, '__CURRENT__').replace(historical, current).replace('__CURRENT__', historical),
    );
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /canonical current cursor must be listed before/);
});

test('rejects an unrecorded advance of the historical embedded slice list', () => {
  const result = rejects((root) => {
    const relativePath = 'crates/rustok-blog/docs/implementation-plan.md';
    const source = readFileSync(absolute(root, relativePath), 'utf8');
    write(
      root,
      relativePath,
      source.replace(
        '\n## Next results',
        '\n68. Unrecorded historical-list advance.\n\n## Next results',
      ),
    );
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /historical embedded completed-slice list unexpectedly advanced/);
});
