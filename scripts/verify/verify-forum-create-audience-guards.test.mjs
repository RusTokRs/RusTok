import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = fileURLToPath(new URL('../../', import.meta.url));
const forum = 'crates/modules/rustok-forum';
const cases = [
  ['category-audience-policy', 'category_audience_owner.rs', 'self.inner.taxonomy_port.as_ref(),'],
  ['topic-audience-policy', 'topic_audience.rs', 'self.inner.taxonomy_port.as_ref(),'],
  ['moderation-audience-policy', 'moderation_audience_authorization.rs', 'self.taxonomy_port.as_ref(),'],
  ['topic-create-audience-transport-composition', '../controllers/content_commands.rs', '.create_command_with_audience_context('],
  ['reply-create-audience-transport-composition', '../controllers/content_commands.rs', '.create_command_with_audience_context('],
  ['category-topic-create-audience-policy', 'category_topic_create_audience.rs', 'load_category_ancestor_ids_with_port('],
  ['category-reply-create-audience-policy', 'category_reply_create_audience.rs', 'load_category_ancestor_ids_with_port('],
  ['topic-reply-create-audience-policy', 'reply_create_audience_authorization.rs', 'load_topic_reply_create_audience_policy_for_topic('],
  ['topic-create-audience-enforcement', 'topic.rs', '.require_in_tx(&txn, tenant_id, input.category_id, &security, context)'],
  ['reply-create-audience-enforcement', 'reply_owner.rs', '.require_in_tx(&txn, tenant_id, &topic, &security, context)'],
];

function snapshot(name, directory) {
  const pending = [
    `scripts/verify/verify-forum-${name}.mjs`,
    `${forum}/contracts/forum-${name}.json`,
  ];
  const copied = new Set();
  while (pending.length) {
    const relative = pending.pop();
    if (copied.has(relative)) continue;
    copied.add(relative);
    const absolute = path.join(root, relative);
    if (!existsSync(absolute) || !statSync(absolute).isFile()) continue;
    const source = readFileSync(absolute, 'utf8');
    const target = path.join(directory, relative);
    mkdirSync(path.dirname(target), { recursive: true });
    writeFileSync(target, source);
    // Follow each guard/contract's real source-of-truth paths, not fabricated markers.
    for (const match of source.matchAll(/["']((?:apps|crates|docs|DECISIONS)\/[^"'\n]+)["']/g)) {
      pending.push(match[1]);
    }
  }
}

for (const [name, file, marker] of cases) {
  for (const mutation of ['current', 'missing-gate', 'missing-owner']) {
    test(`${name} source guard: ${mutation}`, t => {
      const directory = mkdtempSync(path.join(tmpdir(), 'forum-create-guard-'));
      t.after(() => rmSync(directory, { recursive: true, force: true }));
      snapshot(name, directory);
      const target = path.join(directory, forum, 'src/services', file);
      assert.ok(existsSync(target), `snapshot includes live ${file}`);
      if (mutation === 'missing-gate') {
        const source = readFileSync(target, 'utf8');
        assert.ok(source.includes(marker), `fixture mutates actual gate ${marker}`);
        writeFileSync(target, source.replaceAll(marker, 'missing_inherited_create_gate'));
      } else if (mutation === 'missing-owner') {
        rmSync(target);
      }
      const result = spawnSync(process.execPath, [`scripts/verify/verify-forum-${name}.mjs`], {
        cwd: root,
        encoding: 'utf8',
        env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: directory },
      });
      assert.equal(result.signal, null);
      if (mutation === 'current') assert.equal(result.status, 0, result.stdout + result.stderr);
      else {
        assert.equal(result.status, 1, result.stdout + result.stderr);
        assert.match(result.stderr, /verification failed/);
      }
    });
  }
}
