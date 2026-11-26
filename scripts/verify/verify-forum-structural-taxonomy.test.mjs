import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { spaced } from './lib/forum-source-verification.mjs';
import { STRUCTURAL_LOCK_RULES, inspectStructuralLockOrder } from './verify-forum-taxonomy-wiring.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));
for (const rule of STRUCTURAL_LOCK_RULES) {
  const [file, label, start, end, markers] = rule;
  const source = readFileSync(path.join(root, file), 'utf8');
  const from = source.indexOf(start);
  const to = source.indexOf(end, from + start.length);
  const body = spaced(source.slice(from, to));
  for (const mutation of ['current', 'missing lock', 'late lock', 'replay after lock']) {
    test(`${label}: ${mutation}`, () => {
      assert.ok(from >= 0 && to > from);
      let changed = body;
      const replay = spaced(markers[0]);
      const lock = spaced(markers[1]);
      assert.ok(changed.includes(lock));
      if (mutation === 'missing lock') changed = changed.replace(lock, 'missing_hierarchy_lock();');
      if (mutation === 'late lock') changed = changed.replace(lock, '') + lock;
      if (mutation === 'replay after lock') changed = lock + changed.replace(lock, '');
      const errors = inspectStructuralLockOrder(source.slice(0, from) + changed + '\n' + source.slice(to), rule);
      assert.ok(changed.includes(replay));
      assert.equal(errors.length, mutation === 'current' ? 0 : 1, errors.join('\n'));
    });
  }
}

// Run the existing guards against isolated copies of their live files. Losing a call or owner file
// must fail; historical include names and unrelated consolidated adapter functions are not evidence.
const cases = [
  ['owner-read-visibility', 'crates/modules/rustok-forum/src/services/topic.rs',
    'forum_topic::Column::CategoryId.is_not_in(hidden_category_ids.to_vec())'],
  ['category-owner-read-visibility', 'crates/modules/rustok-forum/src/services/category_owner.rs',
    'enforce_scope(&security, Resource::ForumCategories, access.action())?;'],
  ['topic-route-tombstone-visibility-owner', 'crates/modules/rustok-forum/src/services/topic_route_tombstone_visibility.rs',
    'load_policy_for_topic_with_port(port, txn, txn.into(), tenant_id, topic)'],
  ['topic-move-owner', 'crates/modules/rustok-forum/src/services/topic_move.rs',
    'load_policy_for_topic_with_port(\n'],
];
for (const [guard, owner, marker] of cases) {
  const script = path.join(root, `scripts/verify/verify-forum-${guard}.mjs`);
  const source = readFileSync(script, 'utf8');
  const files = new Set([...source.matchAll(/["']((?:crates|apps)\/[^"']+)["']/g)].map(match => match[1]));
  // Contracts hold the selected owner/selector paths, not the verifier's implementation.
  for (const file of [...files]) {
    if (!file.endsWith('.json')) continue;
    const contract = JSON.parse(readFileSync(path.join(root, file), 'utf8'));
    for (const value of Object.values(contract)) {
      if (typeof value === 'string' && value.startsWith('crates/')) files.add(value);
    }
  }
  assert.ok(files.has(owner), `${guard} must name its live owner`);
  for (const mutation of ['current', 'missing edge', 'missing owner']) {
    test(`live ${guard} guard: ${mutation}`, t => {
      const directory = mkdtempSync(path.join(tmpdir(), 'forum-live-owner-'));
      t.after(() => rmSync(directory, { recursive: true, force: true }));
      for (const file of files) {
        if (mutation === 'missing owner' && file === owner) continue;
        let text = readFileSync(path.join(root, file), 'utf8');
        if (mutation === 'missing edge' && file === owner) {
          assert.ok(text.includes(marker), `mutation must remove a live ${guard} edge`);
          text = text.replace(marker, 'missing_selected_owner_edge(');
        }
        const destination = path.join(directory, file);
        mkdirSync(path.dirname(destination), { recursive: true });
        writeFileSync(destination, text);
      }
      const result = spawnSync(process.execPath, [script], { encoding: 'utf8', cwd: root,
        env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: directory } });
      assert.ifError(result.error);
      assert.equal(result.status, mutation === 'current' ? 0 : 1, result.stdout + result.stderr);
    });
  }
}
