#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const verifier = path.join(repositoryRoot, 'scripts/verify/verify-blog-canonical-plan-current.mjs');
const files = [
  'crates/modules/rustok-blog/contracts/evidence/blog-canonical-plan-current-source.json',
  'crates/modules/rustok-blog/docs/implementation-plan-current.md',
  'crates/modules/rustok-blog/docs/implementation-plan.md',
  'crates/modules/rustok-blog/docs/implementation-plan-slice-97.md',
  'crates/modules/rustok-blog/docs/implementation-plan-slice-98.md',
  'crates/modules/rustok-blog/docs/implementation-plan-slice-99.md',
  'crates/modules/rustok-blog/docs/implementation-plan-slice-100.md',
  'crates/modules/rustok-blog/docs/implementation-plan-slice-101.md',
  'crates/modules/rustok-blog/docs/implementation-plan-slice-102.md',
  'crates/modules/rustok-blog/docs/implementation-plan-slice-103.md',
  'crates/modules/rustok-blog/docs/implementation-plan-slice-104.md',
  'crates/modules/rustok-blog/docs/implementation-plan-slice-105.md',
  'crates/modules/rustok-blog/docs/README.md',
  'crates/modules/rustok-blog/contracts/evidence/blog-comments-tcp-transport.json',
  'crates/modules/rustok-blog/contracts/evidence/blog-comments-tcp-server-adapter.json',
  'crates/modules/rustok-blog/contracts/evidence/blog-comments-tcp-listener-lifecycle.json',
  'crates/modules/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json',
  'crates/modules/rustok-blog/contracts/evidence/blog-comments-storefront-write-surface.json',
  'crates/modules/rustok-blog/contracts/evidence/blog-tag-pagination-source.json',
  'crates/modules/rustok-blog/contracts/evidence/blog-tag-canonical-projection-source.json',
  'crates/modules/rustok-blog/contracts/evidence/blog-tag-mutation-reindex-source.json',
  'crates/modules/rustok-blog/contracts/evidence/blog-post-category-name-projection-source.json',
];
function absolute(root, relativePath) { return path.join(root, relativePath); }
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
  try { return run(root); }
  finally { rmSync(root, { recursive: true, force: true }); }
}
/// Rewrites one section: the markers bracket it and the mutator has to change it, so a
/// fixture can never pass by editing a lookalike elsewhere in the document.
function mutateSection(root, relativePath, startMarker, endMarker, mutator) {
  const target = absolute(root, relativePath);
  const source = readFileSync(target, 'utf8');
  const start = source.indexOf(startMarker);
  const end = source.indexOf(endMarker, start + startMarker.length);
  assert.ok(start >= 0 && end > start, `fixture anchors must exist in ${relativePath}`);
  const section = source.slice(start, end);
  const mutated = mutator(section);
  assert.notEqual(mutated, section, `fixture mutation must change ${relativePath}`);
  write(root, relativePath, source.slice(0, start) + mutated + source.slice(end));
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
    assert.match(result.stdout, /cursor=cat17/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('rejects reopening remote transport as live current-cursor work', () => {
  const result = rejects((root) => {
    const file = 'crates/modules/rustok-blog/docs/implementation-plan-current.md';
    const source = readFileSync(absolute(root, file), 'utf8');
    write(root, file, source.replace(
      '`remote_comments_transport = source_implemented_maintainer_execution_pending`',
      '`remote_comments_transport = remote transport remains pending`',
    ));
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /remote transport|current/);
});

test('rejects claiming another source gap without a fresh audit', () => {
  const result = rejects((root) => {
    mutateJson(root, 'crates/modules/rustok-blog/contracts/evidence/blog-canonical-plan-current-source.json', (value) => {
      value.planning_result.independent_production_source_gap_identified = true;
      value.planning_result.next_source_gap = 'another_gap';
      value.planning_result.future_autonomous_source_work_requires_fresh_audit = false;
    });
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /planning\/execution claim drift/);
});

test('rejects reviving the retired Category Translation provider evidence', () => {
  const result = rejects((root) => {
    write(
      root,
      'crates/modules/rustok-blog/contracts/evidence/blog-category-translation-postgres-source.json',
      `${JSON.stringify({ status: 'blog_category_translation_postgres_source_unvalidated' }, null, 2)}\n`,
    );
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /retired provider evidence must remain absent/);
});

test('rejects a storefront write-surface regression to the retired absence claim', () => {
  const result = rejects((root) => {
    mutateJson(root, 'crates/modules/rustok-blog/contracts/evidence/blog-comments-storefront-write-surface.json', (value) => {
      value.status = 'source_verified_absent';
      value.source_contract.comment_form_present = false;
      value.source_contract.create_comment_surface_present = false;
    });
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /write-surface evidence drift/);
});

test('rejects a cached snapshot regression back to planned source work', () => {
  const result = rejects((root) => {
    mutateJson(root, 'crates/modules/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json', (value) => {
      value.storefront_read_degradation.cached_thread_snapshot = 'planned';
    });
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /storefront fallback current-state drift/);
});

test('rejects reopening the tag pagination source gap', () => {
  const result = rejects((root) => {
    mutateJson(root, 'crates/modules/rustok-blog/contracts/evidence/blog-canonical-plan-current-source.json', (value) => {
      value.source_tracks.tag_list_pagination.status = 'planned';
    });
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /tag pagination track drift/);
});

test('rejects dropping the database-side tag pagination claim', () => {
  const result = rejects((root) => {
    mutateJson(root, 'crates/modules/rustok-blog/contracts/evidence/blog-tag-pagination-source.json', (value) => {
      value.source_contract.database_side_pagination_claimed = false;
    });
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /tag pagination source evidence drift/);
});

test('rejects a pagination track that disagrees with its owner evidence', () => {
  const result = rejects((root) => {
    mutateJson(root, 'crates/modules/rustok-blog/contracts/evidence/blog-canonical-plan-current-source.json', (value) => {
      value.source_tracks.tag_list_pagination.maximum_page_size = 1000;
    });
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /tag pagination track maximum_page_size disagrees/);
});

test('rejects metadata tags becoming canonical again', () => {
  const result = rejects((root) => {
    mutateJson(root, 'crates/modules/rustok-blog/contracts/evidence/blog-canonical-plan-current-source.json', (value) => {
      value.source_tracks.tag_canonical_projection.metadata_tags_are_canonical = true;
    });
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /canonical tag projection track drift/);
});

test('rejects reopening atomic tag mutation after slice 104', () => {
  const result = rejects((root) => {
    mutateJson(root, 'crates/modules/rustok-blog/contracts/evidence/blog-canonical-plan-current-source.json', (value) => {
      value.source_tracks.tag_mutation_atomic_reindex.status = 'next_source_gap';
    });
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /tag mutation atomic-reindex track drift/);
});

test('rejects premature tag mutation runtime promotion', () => {
  const result = rejects((root) => {
    mutateJson(root, 'crates/modules/rustok-blog/contracts/evidence/blog-tag-mutation-reindex-source.json', (value) => {
      value.runtime_status = 'validated';
      value.execution.push({ command: 'not-run' });
    });
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /tag mutation atomic-reindex source evidence drift/);
});

test('rejects reopening post category-name projection after slice 105', () => {
  const result = rejects((root) => {
    mutateJson(root, 'crates/modules/rustok-blog/contracts/evidence/blog-canonical-plan-current-source.json', (value) => {
      value.source_tracks.post_category_name_projection.status = 'planned';
    });
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /post category-name projection track drift/);
});

test('rejects premature post category-name runtime promotion', () => {
  const result = rejects((root) => {
    mutateJson(root, 'crates/modules/rustok-blog/contracts/evidence/blog-post-category-name-projection-source.json', (value) => {
      value.runtime_status = 'validated';
      value.execution.push({ command: 'not-run' });
    });
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /post category-name projection source evidence drift/);
});

test('rejects listing the historical plan before the canonical current cursor', () => {
  const result = rejects((root) => {
    const file = 'crates/modules/rustok-blog/docs/README.md';
    const source = readFileSync(absolute(root, file), 'utf8');
    const current = '[Current Implementation Cursor](./implementation-plan-current.md)';
    const historical = '[Historical Implementation Plan](./implementation-plan.md)';
    write(root, file, source.replace(current, '__CURRENT__').replace(historical, current).replace('__CURRENT__', historical));
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /canonical current cursor must be listed before/);
});

test('rejects an unrecorded advance of the historical embedded slice list', () => {
  const result = rejects((root) => {
    mutateSection(
      root,
      'crates/modules/rustok-blog/docs/implementation-plan.md',
      '## Completed implementation slices',
      '## Next results',
      (section) => `${section.trimEnd()}\n73. Unrecorded historical-list advance.\n`,
    );
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /does not match the recorded boundary/);
});

test('rejects a live cursor that keeps a superseded phrase outside the archive', () => {
  const result = rejects((root) => {
    const file = 'crates/modules/rustok-blog/docs/implementation-plan-current.md';
    const source = readFileSync(absolute(root, file), 'utf8');
    write(root, file, source.replace(
      '# rustok-blog canonical implementation cursor',
      '# rustok-blog canonical implementation cursor\n\nremote transport remains pending',
    ));
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /forbidden live cursor marker/);
});

test('rejects dropping an archived superseded phrase', () => {
  const result = rejects((root) => {
    mutateSection(
      root,
      'crates/modules/rustok-blog/docs/implementation-plan-current.md',
      '## Superseded historical cursor phrases',
      '## Validation boundary',
      (section) => section.replace(
        '- `Status: `canonical_source_cursor_actualized_through_slice_105`.`\n',
        '',
      ),
    );
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /archive: missing/);
});

test('rejects remote transport operation parity regression', () => {
  const result = rejects((root) => {
    mutateJson(root, 'crates/modules/rustok-blog/contracts/evidence/blog-comments-tcp-server-adapter.json', (value) => {
      value.operations = value.operations.filter((operation) => operation !== 'delete_comment');
    });
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /seven-operation transport parity drift/);
});
