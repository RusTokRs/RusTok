import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { compileFunction } from 'node:vm';
import test from 'node:test';

// Execute the production transport function with only the shared gateway injected.
// No Next host or real network is needed to exercise the complete cursor loop.
const source = readFileSync(new URL('./forum.ts', import.meta.url), 'utf8');
const listSource = source
  .slice(
    source.indexOf('export async function listForumTopics('),
    source.indexOf('/** Upper bound for admin operations')
  )
  .replace('export async function', 'async function');
const loadTopics = (gateway) =>
  compileFunction(
    `${stripTypeScriptTypes(listSource)}\nreturn listForumTopics;`,
    ['graphqlRequest']
  )(gateway);

test('admin selectors load all topics beyond 100 with the same tenant, auth and locale', async () => {
  const all = Array.from({ length: 237 }, (_, n) => ({ id: `topic-${n}` }));
  const calls = [];
  const list = loadTopics(async (query, variables, token, tenant) => {
    assert.match(query, /nextCursor/);
    assert.match(query, /after: \$after/);
    assert.equal(variables.tenantId, 'tenant-id');
    assert.equal(variables.locale, 'ru');
    assert.equal(variables.perPage, 100);
    assert.equal(token, 'token');
    assert.equal(tenant, 'tenant-slug');
    calls.push(variables.after);
    const offset = Number(variables.after ?? 0);
    return {
      forumTopics: {
        items: all.slice(offset, offset + 100),
        nextCursor: offset + 100 < all.length ? String(offset + 100) : null
      }
    };
  });
  assert.deepEqual(
    await list(
      { tenantId: 'tenant-id', token: 'token', tenantSlug: 'tenant-slug' },
      { locale: 'ru' }
    ),
    all
  );
  assert.deepEqual(calls, [null, '100', '200']);
});

test('a cursor overlap does not duplicate topics moved by concurrent writes', async () => {
  let call = 0;
  const list = loadTopics(async () => ({
    forumTopics:
      ++call === 1
        ? { items: [{ id: 'a' }, { id: 'b' }], nextCursor: 'next' }
        : { items: [{ id: 'b' }, { id: 'c' }], nextCursor: null }
  }));
  assert.deepEqual(await list(), [{ id: 'a' }, { id: 'b' }, { id: 'c' }]);
});

test('empty forums return an empty selector', async () => {
  const list = loadTopics(async () => ({
    forumTopics: { items: [], nextCursor: null }
  }));
  assert.deepEqual(await list(), []);
});

test('non-advancing cursors fail explicitly rather than hanging or returning partial candidates', async () => {
  for (const nextCursor of ['stuck', '', undefined]) {
    const list = loadTopics(async () => ({
      forumTopics: { items: [{ id: 'a' }], nextCursor }
    }));
    await assert.rejects(list(), /pagination did not advance/);
  }
  const list = loadTopics(async () => ({
    forumTopics: { items: [], nextCursor: 'next' }
  }));
  await assert.rejects(list(), /pagination did not advance/);
});

test('a failure after the first page propagates instead of silently returning 100 topics', async () => {
  let call = 0;
  const list = loadTopics(async () => {
    if (++call === 2) throw new Error('owner unavailable');
    return { forumTopics: { items: [{ id: 'a' }], nextCursor: 'next' } };
  });
  await assert.rejects(list(), /owner unavailable/);
});
