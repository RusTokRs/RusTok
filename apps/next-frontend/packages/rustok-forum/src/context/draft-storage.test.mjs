import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { getDraftIdentity, getDraftStorageKey, isCurrentDraftSession, switchTopicDraft } from './draft-storage.ts';

const alice = '51c31654-2eb3-4764-b3d6-f912fc9f36e3';
const bob = 'b48848d2-4c20-4f86-af04-d22558c4cb4b';
const tenantA = '967fb11e-fd28-4722-9b99-f05af5174d68';
const tenantB = 'e650359e-b20c-460d-99e2-65570709c87d';

function token(claims = {}) {
  return `header.${Buffer.from(JSON.stringify({
    sub: alice, tenant_id: tenantA, exp: Math.floor(Date.now() / 1000) + 3600,
    grant_type: 'direct', ...claims,
  })).toString('base64url')}.signature`;
}

test('draft identity follows the access-token subject, never a stale user record', () => {
  assert.deepEqual(getDraftIdentity(token()), { userId: alice, tenantId: tenantA });
  assert.deepEqual(getDraftIdentity(token({ sub: bob, tenant_id: tenantB })), {
    userId: bob, tenantId: tenantB,
  });
  assert.deepEqual(getDraftIdentity(token({ grant_type: 'authorization_code' })), {
    userId: alice, tenantId: tenantA,
  });
  for (const invalid of [
    '', 'not-a-jwt', 'header.%%%%.signature',
    token({ sub: 'alice' }), token({ tenant_id: 'invalid' }),
    token({ grant_type: 'client_credentials' }),
    token({ exp: Math.floor(Date.now() / 1000) - 10 }),
  ]) assert.equal(getDraftIdentity(invalid), null);
});

test('draft storage keys have unambiguous route, token tenant, user, mode, and target scopes', () => {
  const base = { tenant: 'tenant-a', tenantId: tenantA, userId: alice };
  const current = getDraftStorageKey(base, 'topic', null, 'general');
  for (const other of [
    getDraftStorageKey({ ...base, tenant: 'tenant-b' }, 'topic', null, 'general'),
    getDraftStorageKey({ ...base, tenantId: tenantB }, 'topic', null, 'general'),
    getDraftStorageKey({ ...base, userId: bob }, 'topic', null, 'general'),
    getDraftStorageKey(base, 'topic', null, 'other'),
    getDraftStorageKey(base, 'reply', 'general', null),
    getDraftStorageKey({ ...base, tenant: 'a:b', userId: 'c' }, 'topic', null, null),
    getDraftStorageKey({ ...base, tenant: 'a', userId: 'b:c' }, 'topic', null, null),
  ]) assert.notEqual(current, other);
  assert.notEqual(current, 'rustok_forum_draft_topic_general');
  assert.notEqual(current, 'rustok_forum_draft_reply_general');
});

test('the topic composer submits its displayed selection and updates the draft scope', () => {
  const composer = readFileSync(new URL('../components/composer.tsx', import.meta.url), 'utf8');
  const context = readFileSync(new URL('./composer-context.tsx', import.meta.url), 'utf8');
  assert.match(composer, /onChange=\{\(e\) => handleCategoryChange\(e\.target\.value\)\}/);
  assert.match(composer, /const result = selectCategory\(nextCategoryId\)/);
  assert.match(composer, /const cId = selectedCategoryId;/);
  assert.doesNotMatch(composer, /const cId = state\.categoryId \|\|/);
  assert.doesNotMatch(composer, /: categories\[0\]\?\.id/);
  assert.match(context, /switchTopicDraft\(localStorage, scope, categoryId, nextCategoryId/);
});

test('changing posting category moves only the current viewer\'s draft before submission', () => {
  const scope = { tenant: 'tenant-a', tenantId: tenantA, userId: alice };
  const other = { ...scope, userId: bob };
  const oldKey = getDraftStorageKey(scope, 'topic', null, 'category-a');
  const newKey = getDraftStorageKey(scope, 'topic', null, 'category-b');
  const otherKey = getDraftStorageKey(other, 'topic', null, 'category-a');
  const entries = new Map([[oldKey, 'old draft'], [otherKey, 'other viewer draft']]);
  const storage = {
    getItem: (key) => entries.get(key) ?? null,
    setItem: (key, value) => entries.set(key, value),
    removeItem: (key) => entries.delete(key),
  };
  const draft = { title: 'New topic', document: { type: 'doc', content: [] }, quotes: [] };
  assert.equal(switchTopicDraft(storage, scope, 'category-a', 'category-b', draft), null);
  assert.equal(entries.has(oldKey), false);
  assert.deepEqual(JSON.parse(entries.get(newKey)), draft);
  assert.equal(entries.get(otherKey), 'other viewer draft');
});

test('switching to an occupied category restores its draft and retains the outgoing one', () => {
  const scope = { tenant: 'tenant-a', tenantId: tenantA, userId: alice };
  const oldKey = getDraftStorageKey(scope, 'topic', null, 'category-a');
  const newKey = getDraftStorageKey(scope, 'topic', null, 'category-b');
  const outgoing = { title: 'Current edits', document: { type: 'doc', content: [] }, quotes: [] };
  const destination = { title: 'Saved earlier', document: { type: 'doc', content: [] }, quotes: [] };
  const entries = new Map([[newKey, JSON.stringify(destination)]]);
  const storage = {
    getItem: (key) => entries.get(key) ?? null,
    setItem: (key, value) => entries.set(key, value),
    removeItem: (key) => entries.delete(key),
  };
  assert.deepEqual(switchTopicDraft(storage, scope, 'category-a', 'category-b', outgoing), destination);
  assert.deepEqual(JSON.parse(entries.get(oldKey)), outgoing);
  assert.deepEqual(JSON.parse(entries.get(newKey)), destination);
  assert.deepEqual(switchTopicDraft(storage, scope, 'category-b', 'category-a', destination), outgoing);
  assert.deepEqual(JSON.parse(entries.get(newKey)), destination);
});

test('a failed write or corrupt destination preserves both existing drafts', () => {
  const scope = { tenant: 'tenant-a', tenantId: tenantA, userId: alice };
  const oldKey = getDraftStorageKey(scope, 'topic', null, null);
  const newKey = getDraftStorageKey(scope, 'topic', null, 'new-category');
  const entries = new Map([[oldKey, 'old draft']]);
  const storage = {
    getItem: (key) => entries.get(key) ?? null,
    setItem: () => { throw new Error('quota'); },
    removeItem: (key) => entries.delete(key),
  };
  const outgoing = { title: 'Unsent', document: { type: 'doc', content: [] }, quotes: [] };
  assert.throws(() => switchTopicDraft(storage, scope, null, 'new-category', outgoing), /quota/);
  assert.equal(entries.get(oldKey), 'old draft');
  assert.equal(entries.has(newKey), false);
  entries.set(newKey, '{corrupt');
  assert.throws(() => switchTopicDraft(storage, scope, null, 'new-category', outgoing), /JSON|Unexpected/);
  assert.equal(entries.get(oldKey), 'old draft');
  assert.equal(entries.get(newKey), '{corrupt');
  entries.set(newKey, JSON.stringify({ title: 'Bad quotes', document: outgoing.document, quotes: [null] }));
  assert.throws(() => switchTopicDraft(storage, scope, null, 'new-category', outgoing), /Invalid saved topic draft/);
  assert.equal(entries.get(oldKey), 'old draft');
});

test('closing flushes edits before cancelling the autosave timer and storage failures are visible', () => {
  const context = readFileSync(new URL('./composer-context.tsx', import.meta.url), 'utf8');
  const composer = readFileSync(new URL('../components/composer.tsx', import.meta.url), 'utf8');
  assert.match(context, /const close = useCallback\(\(\) => \{[\s\S]*?if \(isOpen\) persistDraft\(\);/);
  assert.match(context, /window\.addEventListener\('pagehide', flush\);[\s\S]*?return \(\) => \{\s*flush\(\);/);
  assert.match(context, /const reset = useCallback\(\(\) => \{\s*activeDraftRef\.current = false;\s*clearDraft\(/);
  for (const method of ['openReply', 'openNewTopic']) {
    assert.match(context, new RegExp(`const ${method} = useCallback\\([\\s\\S]*?if \\(isSubmitting\\) return;[\\s\\S]*?if \\(isOpen && scope && !persistDraft\\(\\)\\) return;`));
  }
  assert.match(context, /setDraftStorageStatus\('saved'\)/);
  assert.match(context, /setDraftStorageStatus\('failed'\)/);
  assert.match(composer, /state\.draftStorage === 'failed' \? forum\('draftSaveFailed'\)/);
});

test('an in-flight save or submit cannot cross a viewer or tenant switch', () => {
  const scope = { tenant: 'tenant-a', tenantId: tenantA, userId: alice };
  const expected = { token: token(), tenant: 'tenant-a', scope };
  assert.equal(isCurrentDraftSession(expected, {
    token: expected.token, tenant: 'tenant-a', identity: getDraftIdentity(expected.token),
  }), true);
  assert.equal(isCurrentDraftSession(expected, {
    token: token({ jti: 'other-session' }), tenant: 'tenant-a', identity: { userId: alice, tenantId: tenantA },
  }), false);
  assert.equal(isCurrentDraftSession(expected, {
    token: expected.token, tenant: 'tenant-b', identity: { userId: alice, tenantId: tenantA },
  }), false);
  assert.equal(isCurrentDraftSession(expected, {
    token: expected.token, tenant: 'tenant-a', identity: { userId: bob, tenantId: tenantA },
  }), false);
  assert.equal(isCurrentDraftSession(expected, {
    token: expected.token, tenant: 'tenant-a', identity: { userId: alice, tenantId: tenantB },
  }), false);
  assert.equal(isCurrentDraftSession({ ...expected, token: '' }, {
    token: '', tenant: 'tenant-a', identity: { userId: alice, tenantId: tenantA },
  }), false);
});
