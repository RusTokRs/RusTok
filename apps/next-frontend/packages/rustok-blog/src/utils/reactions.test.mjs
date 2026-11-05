import assert from 'node:assert/strict';
import { test } from 'node:test';
import { blogReactionSubject, decimalU64, readReactionSnapshot, reactionControls, settleReactionMutation } from './reactions.ts';
const post = 'c293edb7-4f5b-432b-9011-096354ba8ba6';
test('reaction subject requires the real positive owner revision', () => {
  assert.equal(blogReactionSubject(post, 42).subjectRevision, '42');
  for (const revision of [undefined, 0, -1, NaN, 1.5, Infinity]) assert.throws(() => blogReactionSubject(post, revision));
  assert.throws(() => blogReactionSubject('00000000-0000-0000-0000-000000000000', 1));
});
test('owner counters retain full u64 precision and reject lossy wire values', () => {
  assert.equal(decimalU64('18446744073709551615'), '18446744073709551615');
  for (const value of [9007199254740992, '18446744073709551616', '-1', '01', '1e3', '']) assert.throws(() => decimalU64(value));
});
test('catalog and actor state determine controls without a shadow aggregate', () => {
  const snapshot = readReactionSnapshot({ catalog: { keys: ['like'], selectionMode: 'SINGLE', maxSelected: 1 }, actorState: { revision: '4', selected: ['like'] }, aggregates: [{ reaction: 'like', count: '9007199254740993' }] });
  assert.deepEqual(reactionControls(snapshot), [{ reaction: 'like', selected: true, count: '9007199254740993', canMutate: true }]);
  assert.equal(reactionControls({ ...snapshot, actorState: null })[0].canMutate, false);
  assert.throws(() => readReactionSnapshot({ ...snapshot, aggregates: [{ reaction: 'like', count: 10 }] }));
});

test('malformed actor/catalog state never enables an anonymous control', () => {
  const base = { catalog: { keys: ['like'], selectionMode: 'SINGLE', maxSelected: 1 }, actorState: null, aggregates: [] };
  for (const value of [{ ...base, actorState: undefined }, { ...base, actorState: {} }, { ...base, catalog: { ...base.catalog, keys: ['like', 'like'] } }, { ...base, actorState: { revision: '1', selected: ['foreign'] } }]) assert.throws(() => readReactionSnapshot(value));
});

test('uncertain mutation reloads canonical state before reporting completion', async () => {
  const calls = []; const snapshot = { catalog: { keys: ['like'], selectionMode: 'SINGLE', maxSelected: 1 }, actorState: null, aggregates: [{ reaction: 'like', count: '9007199254740993' }] };
  const result = await settleReactionMutation(async () => { calls.push('write'); throw new Error('timeout after commit'); }, async () => { calls.push('read'); return snapshot; });
  assert.deepEqual(calls, ['write', 'read']); assert.equal(result.writeFailed, true); assert.equal(result.snapshot, snapshot);
  await assert.rejects(settleReactionMutation(async () => {}, async () => { throw new Error('snapshot unavailable'); }));
});
