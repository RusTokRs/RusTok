import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { stripTypeScriptTypes } from 'node:module';
import { compileFunction } from 'node:vm';
import { validateRichTextDocument } from '../../../../../../packages/richtext/src/document.ts';
import { RICH_TEXT_PROFILES } from '../../../../../../packages/richtext/src/generated/profiles.ts';

// Run the real shared validator/profile implementation and production draft module.
// Strip host package imports only: Node's test runner cannot resolve the Next package aliases.
function executableModule(path) {
  return stripTypeScriptTypes(
    readFileSync(new URL(path, import.meta.url), 'utf8'),
  )
    .replace(/^import[\s\S]*?from ['"][^'"]+['"];?\s*/gm, '')
    .replace(/^export \{[^}]+\};?\s*/gm, '')
    .replace(/^export /gm, '');
}
const getRichTextProfile = compileFunction(
  `${executableModule('../../../../../../packages/richtext/src/profiles.ts')}\nreturn getRichTextProfile;`,
  ['RICH_TEXT_PROFILES'],
)(RICH_TEXT_PROFILES);
const draftStorage = compileFunction(
  `${executableModule('./draft-storage.ts')}\nreturn {getDraftIdentity, getDraftStorageKey, isCurrentDraftSession, switchTopicDraft, parseTopicDraft, parseReplyDraft};`,
  ['getRichTextProfile', 'validateRichTextDocument'],
)(getRichTextProfile, validateRichTextDocument);
const {
  getDraftIdentity,
  getDraftStorageKey,
  isCurrentDraftSession,
  switchTopicDraft,
} = draftStorage;

const alice = '51c31654-2eb3-4764-b3d6-f912fc9f36e3';
const bob = 'b48848d2-4c20-4f86-af04-d22558c4cb4b';
const tenantA = '967fb11e-fd28-4722-9b99-f05af5174d68';
const tenantB = 'e650359e-b20c-460d-99e2-65570709c87d';

function token(claims = {}) {
  return `header.${Buffer.from(
    JSON.stringify({
      sub: alice,
      tenant_id: tenantA,
      exp: Math.floor(Date.now() / 1000) + 3600,
      grant_type: 'direct',
      ...claims,
    }),
  ).toString('base64url')}.signature`;
}

test('draft identity follows the access-token subject, never a stale user record', () => {
  assert.deepEqual(getDraftIdentity(token()), {
    userId: alice,
    tenantId: tenantA,
  });
  assert.deepEqual(getDraftIdentity(token({ sub: bob, tenant_id: tenantB })), {
    userId: bob,
    tenantId: tenantB,
  });
  assert.deepEqual(
    getDraftIdentity(token({ grant_type: 'authorization_code' })),
    {
      userId: alice,
      tenantId: tenantA,
    },
  );
  for (const invalid of [
    '',
    'not-a-jwt',
    'header.%%%%.signature',
    token({ sub: 'alice' }),
    token({ tenant_id: 'invalid' }),
    token({ grant_type: 'client_credentials' }),
    token({ exp: Math.floor(Date.now() / 1000) - 10 }),
  ])
    assert.equal(getDraftIdentity(invalid), null);
});

test('draft storage keys have unambiguous route, token tenant, user, mode, and target scopes', () => {
  const base = { tenant: 'tenant-a', tenantId: tenantA, userId: alice };
  const current = getDraftStorageKey(base, 'topic', null, 'general');
  for (const other of [
    getDraftStorageKey(
      { ...base, tenant: 'tenant-b' },
      'topic',
      null,
      'general',
    ),
    getDraftStorageKey(
      { ...base, tenantId: tenantB },
      'topic',
      null,
      'general',
    ),
    getDraftStorageKey({ ...base, userId: bob }, 'topic', null, 'general'),
    getDraftStorageKey(base, 'topic', null, 'other'),
    getDraftStorageKey(base, 'reply', 'general', null),
    getDraftStorageKey(
      { ...base, tenant: 'a:b', userId: 'c' },
      'topic',
      null,
      null,
    ),
    getDraftStorageKey(
      { ...base, tenant: 'a', userId: 'b:c' },
      'topic',
      null,
      null,
    ),
  ])
    assert.notEqual(current, other);
  assert.notEqual(current, 'rustok_forum_draft_topic_general');
  assert.notEqual(current, 'rustok_forum_draft_reply_general');
});

test('the topic composer submits its displayed selection and updates the draft scope', () => {
  const composer = readFileSync(
    new URL('../components/composer.tsx', import.meta.url),
    'utf8',
  );
  const context = readFileSync(
    new URL('./composer-context.tsx', import.meta.url),
    'utf8',
  );
  assert.match(
    composer,
    /onChange=\{\(e\) => handleCategoryChange\(e\.target\.value\)\}/,
  );
  assert.match(composer, /const result = selectCategory\(nextCategoryId\)/);
  assert.match(composer, /const cId = selectedCategoryId;/);
  assert.doesNotMatch(composer, /const cId = state\.categoryId \|\|/);
  assert.doesNotMatch(composer, /: categories\[0\]\?\.id/);
  assert.match(
    context,
    /switchTopicDraft\(localStorage, scope, categoryId, nextCategoryId/,
  );
});

test("changing posting category moves only the current viewer's draft before submission", () => {
  const scope = { tenant: 'tenant-a', tenantId: tenantA, userId: alice };
  const other = { ...scope, userId: bob };
  const oldKey = getDraftStorageKey(scope, 'topic', null, 'category-a');
  const newKey = getDraftStorageKey(scope, 'topic', null, 'category-b');
  const otherKey = getDraftStorageKey(other, 'topic', null, 'category-a');
  const entries = new Map([
    [oldKey, 'old draft'],
    [otherKey, 'other viewer draft'],
  ]);
  const storage = {
    getItem: (key) => entries.get(key) ?? null,
    setItem: (key, value) => entries.set(key, value),
    removeItem: (key) => entries.delete(key),
  };
  const draft = {
    title: 'New topic',
    document: { type: 'doc', content: [] },
    quotes: [],
  };
  assert.equal(
    switchTopicDraft(storage, scope, 'category-a', 'category-b', draft),
    null,
  );
  assert.equal(entries.has(oldKey), false);
  assert.deepEqual(JSON.parse(entries.get(newKey)), draft);
  assert.equal(entries.get(otherKey), 'other viewer draft');
});

test('switching to an occupied category restores its draft and retains the outgoing one', () => {
  const scope = { tenant: 'tenant-a', tenantId: tenantA, userId: alice };
  const oldKey = getDraftStorageKey(scope, 'topic', null, 'category-a');
  const newKey = getDraftStorageKey(scope, 'topic', null, 'category-b');
  const outgoing = {
    title: 'Current edits',
    document: { type: 'doc', content: [] },
    quotes: [],
  };
  const destination = {
    title: 'Saved earlier',
    document: { type: 'doc', content: [] },
    quotes: [],
  };
  const entries = new Map([[newKey, JSON.stringify(destination)]]);
  const storage = {
    getItem: (key) => entries.get(key) ?? null,
    setItem: (key, value) => entries.set(key, value),
    removeItem: (key) => entries.delete(key),
  };
  assert.deepEqual(
    switchTopicDraft(storage, scope, 'category-a', 'category-b', outgoing),
    destination,
  );
  assert.deepEqual(JSON.parse(entries.get(oldKey)), outgoing);
  assert.deepEqual(JSON.parse(entries.get(newKey)), destination);
  assert.deepEqual(
    switchTopicDraft(storage, scope, 'category-b', 'category-a', destination),
    outgoing,
  );
  assert.deepEqual(JSON.parse(entries.get(newKey)), destination);
});

test('a failed write or corrupt destination preserves both existing drafts', () => {
  const scope = { tenant: 'tenant-a', tenantId: tenantA, userId: alice };
  const oldKey = getDraftStorageKey(scope, 'topic', null, null);
  const newKey = getDraftStorageKey(scope, 'topic', null, 'new-category');
  const entries = new Map([[oldKey, 'old draft']]);
  const storage = {
    getItem: (key) => entries.get(key) ?? null,
    setItem: () => {
      throw new Error('quota');
    },
    removeItem: (key) => entries.delete(key),
  };
  const outgoing = {
    title: 'Unsent',
    document: { type: 'doc', content: [] },
    quotes: [],
  };
  assert.throws(
    () => switchTopicDraft(storage, scope, null, 'new-category', outgoing),
    /quota/,
  );
  assert.equal(entries.get(oldKey), 'old draft');
  assert.equal(entries.has(newKey), false);
  entries.set(newKey, '{corrupt');
  assert.throws(
    () => switchTopicDraft(storage, scope, null, 'new-category', outgoing),
    /JSON|Unexpected/,
  );
  assert.equal(entries.get(oldKey), 'old draft');
  assert.equal(entries.get(newKey), '{corrupt');
  entries.set(
    newKey,
    JSON.stringify({
      title: 'Bad quotes',
      document: outgoing.document,
      quotes: [null],
    }),
  );
  assert.throws(
    () => switchTopicDraft(storage, scope, null, 'new-category', outgoing),
    /Invalid saved topic draft/,
  );
  assert.equal(entries.get(oldKey), 'old draft');
});

test('closing flushes edits before cancelling the autosave timer and storage failures are visible', () => {
  const context = readFileSync(
    new URL('./composer-context.tsx', import.meta.url),
    'utf8',
  );
  const composer = readFileSync(
    new URL('../components/composer.tsx', import.meta.url),
    'utf8',
  );
  assert.match(
    context,
    /const close = useCallback\(\(\) => \{[\s\S]*?if \(isOpen && scope && !persistDraft\(\)\) return;/,
  );
  assert.match(
    context,
    /window\.addEventListener\('pagehide', flush\);[\s\S]*?return \(\) => \{\s*flush\(\);/,
  );
  assert.match(
    context,
    /const reset = useCallback\(\(\) => \{\s*activeDraftRef\.current = false;\s*clearDraft\(/,
  );
  for (const method of ['openReply', 'openNewTopic']) {
    const start = context.indexOf(`  const ${method} = useCallback(`);
    const end = context.indexOf('\n  const ', start + 1);
    const opening = context.slice(start, end);
    assert.match(
      opening,
      /if \(isSubmitting \|\| submissionPendingRef\.current \|\| !isCurrentViewer\(\)\) return(?: false)?;/,
    );
    assert.match(
      opening,
      /if \(isOpen && scope && !persistDraft\(\)\) return(?: false)?;/,
    );
  }
  assert.match(context, /setDraftStorageStatus\('saved'\)/);
  assert.match(context, /setDraftStorageStatus\('failed'\)/);
  assert.match(
    composer,
    /state\.draftStorage === 'failed' \? forum\('draftSaveFailed'\)/,
  );
});

test('an in-flight save or submit cannot cross a viewer or tenant switch', () => {
  const scope = { tenant: 'tenant-a', tenantId: tenantA, userId: alice };
  const expected = { token: token(), tenant: 'tenant-a', scope };
  assert.equal(
    isCurrentDraftSession(expected, {
      token: expected.token,
      tenant: 'tenant-a',
      identity: getDraftIdentity(expected.token),
    }),
    true,
  );
  assert.equal(
    isCurrentDraftSession(expected, {
      token: token({ jti: 'other-session' }),
      tenant: 'tenant-a',
      identity: { userId: alice, tenantId: tenantA },
    }),
    false,
  );
  assert.equal(
    isCurrentDraftSession(expected, {
      token: expected.token,
      tenant: 'tenant-b',
      identity: { userId: alice, tenantId: tenantA },
    }),
    false,
  );
  assert.equal(
    isCurrentDraftSession(expected, {
      token: expected.token,
      tenant: 'tenant-a',
      identity: { userId: bob, tenantId: tenantA },
    }),
    false,
  );
  assert.equal(
    isCurrentDraftSession(expected, {
      token: expected.token,
      tenant: 'tenant-a',
      identity: { userId: alice, tenantId: tenantB },
    }),
    false,
  );
  assert.equal(
    isCurrentDraftSession(
      { ...expected, token: '' },
      {
        token: '',
        tenant: 'tenant-a',
        identity: { userId: alice, tenantId: tenantA },
      },
    ),
    false,
  );
});

test('reply drafts are isolated by parent including top-level replies to the same topic', () => {
  const scope = { tenant: 'tenant-a', tenantId: tenantA, userId: alice };
  const keys = [null, 'reply-a', 'reply-b', 'null'].map((parent) =>
    getDraftStorageKey(scope, 'reply', 'topic-a', null, parent),
  );
  assert.equal(new Set(keys).size, 4);
  const oldTopicOnlyKey = `rustok_forum_draft_${encodeURIComponent(
    JSON.stringify([
      scope.tenant,
      scope.tenantId,
      scope.userId,
      'reply',
      'topic-a',
    ]),
  )}`;
  for (const key of keys) assert.notEqual(key, oldTopicOnlyKey);
  assert.notEqual(
    keys[1],
    getDraftStorageKey(
      { ...scope, userId: bob },
      'reply',
      'topic-a',
      null,
      'reply-a',
    ),
  );
  assert.notEqual(
    keys[1],
    getDraftStorageKey(scope, 'reply', 'topic-b', null, 'reply-a'),
  );
});

test('reload restores only a validated draft with the selected reply parent', async () => {
  const { parseReplyDraft } = draftStorage;
  const scope = { tenant: 'tenant-a', tenantId: tenantA, userId: alice };
  const entries = new Map();
  for (const parent of [null, 'reply-a', 'reply-b']) {
    const draft = {
      title: '',
      document: {
        type: 'doc',
        content: [
          {
            type: 'paragraph',
            content: [{ type: 'text', text: `For ${parent}` }],
          },
        ],
      },
      quotes: [],
      parentReplyId: parent,
    };
    const key = getDraftStorageKey(scope, 'reply', 'topic-a', null, parent);
    entries.set(key, JSON.stringify(draft));
    assert.deepEqual(parseReplyDraft(entries.get(key), parent), draft);
  }
  const parentAKey = getDraftStorageKey(
    scope,
    'reply',
    'topic-a',
    null,
    'reply-a',
  );
  assert.throws(
    () => parseReplyDraft(entries.get(parentAKey), 'reply-b'),
    /parent does not match/,
  );
  assert.throws(
    () => parseReplyDraft(entries.get(parentAKey), null),
    /parent does not match/,
  );
  const unrecordedParent = JSON.stringify({
    title: '',
    document: { type: 'doc', content: [] },
    quotes: [],
  });
  assert.throws(
    () => parseReplyDraft(unrecordedParent, null),
    /parent does not match/,
  );
  assert.throws(
    () => parseReplyDraft('{invalid', 'reply-a'),
    /JSON|Unexpected/,
  );
});

test('reply draft load, autosave and successful cleanup share the complete parent namespace', () => {
  const context = readFileSync(
    new URL('./composer-context.tsx', import.meta.url),
    'utf8',
  );
  assert.match(context, /loadDraft\('reply', params\.topicId, null, pId\)/);
  assert.match(
    context,
    /getDraftStorageKey\(scope, mode, topicId, categoryId, parentReplyId\)/,
  );
  assert.match(
    context,
    /getDraftStorageKey\(scope, draftMode, tId, cId, pId\)/,
  );
  assert.match(
    context,
    /clearDraft\(mode, topicId, categoryId, parentReplyId\)/,
  );
  assert.match(context, /\{ title, document, quotes, parentReplyId \}/);
  assert.match(context, /parseReplyDraft\(saved, pId\)/);
  assert.doesNotMatch(context, /const parsed = JSON\.parse\(saved\)/);
});

async function contextCallback(name, end, bindings) {
  bindings = {
    submissionPendingRef: { current: false },
    editorSessionRef: { current: 0 },
    ...bindings,
  };
  if (name !== 'advanceEditorSession' && !bindings.advanceEditorSession) {
    bindings.advanceEditorSession = await contextCallback(
      'advanceEditorSession',
      '\n  const setSubmitting',
      {
        editorSessionRef: bindings.editorSessionRef,
        setEditorSession: bindings.setEditorSession ?? (() => {}),
      },
    );
  }
  const { stripTypeScriptTypes } = await import('node:module');
  const { compileFunction } = await import('node:vm');
  const source = readFileSync(
    new URL('./composer-context.tsx', import.meta.url),
    'utf8',
  );
  const start = source.indexOf(`  const ${name} = useCallback(`);
  const finish = source.indexOf(end, start);
  assert.ok(start >= 0 && finish > start);
  return compileFunction(
    `${stripTypeScriptTypes(source.slice(start, finish))}\nreturn ${name};`,
    ['useCallback', ...Object.keys(bindings)],
  )((fn) => fn, ...Object.values(bindings));
}

test('production reply save, load and cleanup preserve other parents in storage', async () => {
  const { parseReplyDraft, parseTopicDraft } = draftStorage;
  const scope = { tenant: 'tenant-a', tenantId: tenantA, userId: alice };
  const entries = new Map();
  const localStorage = {
    getItem: (key) => entries.get(key) ?? null,
    setItem: (key, value) => entries.set(key, value),
    removeItem: (key) => entries.delete(key),
  };
  const common = {
    scope,
    isCurrentViewer: () => true,
    localStorage,
    getDraftStorageKey,
    pendingDraftCleanupRef: { current: new Set() },
    setDraftStorageStatus: () => {},
  };
  common.retryDraftCleanup = await contextCallback(
    'retryDraftCleanup',
    '\n\n  // Parse before',
    common,
  );
  const docs = [null, 'reply-a', 'reply-b'].map((parentReplyId) => ({
    parentReplyId,
    title: '',
    quotes: [],
    document: {
      type: 'doc',
      content: [
        {
          type: 'paragraph',
          content: [{ type: 'text', text: `For ${parentReplyId}` }],
        },
      ],
    },
  }));
  for (const draft of docs) {
    const persist = await contextCallback('persistDraft', '\n\n  // Autosave', {
      ...common,
      mode: 'reply',
      topicId: 'topic-a',
      categoryId: null,
      ...draft,
      setDraftStorageStatus: (status) => assert.equal(status, 'saved'),
    });
    assert.equal(persist(), true);
  }
  const load = await contextCallback('loadDraft', '\n\n  const persistDraft', {
    ...common,
    parseReplyDraft,
    parseTopicDraft,
  });
  for (const draft of docs)
    assert.deepEqual(
      load('reply', 'topic-a', null, draft.parentReplyId),
      draft,
    );
  const clear = await contextCallback(
    'clearDraft',
    '\n\n  const openReply',
    common,
  );
  clear('reply', 'topic-a', null, 'reply-a');
  assert.equal(load('reply', 'topic-a', null, 'reply-a'), null);
  assert.deepEqual(load('reply', 'topic-a', null, 'reply-b'), docs[2]);
  assert.deepEqual(load('reply', 'topic-a', null, null), docs[0]);
});

test('a corrupt reply destination is rejected without retargeting or clearing current editor state', async () => {
  const statuses = [];
  const activeDraftRef = { current: true };
  const bindings = {
    isSubmitting: false,
    isOpen: true,
    scope: {},
    persistDraft: () => true,
    isCurrentViewer: () => true,
    loadDraft: () => {
      throw new Error('corrupt');
    },
    activeDraftRef,
    setDraftStorageStatus: (status) => statuses.push(status),
    emptyRichTextDocument: () => assert.fail('must not clear document'),
  };
  for (const name of [
    'setMode',
    'setTopicId',
    'setTopicTitle',
    'setCategoryId',
    'setParentReplyId',
    'setTitle',
    'setQuotes',
    'setError',
    'setDocument',
    'setIsOpen',
    'setIsMinimized',
  ]) {
    bindings[name] = () =>
      assert.fail(`${name} must not run before a successful draft read`);
  }
  const open = await contextCallback(
    'openReply',
    '\n\n  const openNewTopic',
    bindings,
  );
  open({ topicId: 'topic-a', topicTitle: 'A', parentReplyId: 'reply-b' });
  assert.deepEqual(statuses, ['failed']);
  assert.equal(activeDraftRef.current, true);
});

test('reply opening acknowledges only a committed target and rejects every failed switch', async () => {
  for (const failure of ['pending', 'viewer', 'persist', 'restore', null]) {
    const changes = [];
    const bindings = {
      isSubmitting: failure === 'pending',
      isOpen: true,
      scope: {},
      isCurrentViewer: () => failure !== 'viewer',
      persistDraft: () => failure !== 'persist',
      loadDraft: () => {
        if (failure === 'restore') throw new Error('unreadable');
        return null;
      },
      activeDraftRef: { current: false },
      emptyRichTextDocument: () => ({ type: 'doc', content: [] }),
      setDraftStorageStatus: () => {},
    };
    for (const name of [
      'setMode',
      'setTopicId',
      'setTopicTitle',
      'setCategoryId',
      'setParentReplyId',
      'setTitle',
      'setQuotes',
      'setError',
      'setDocument',
      'setIsOpen',
      'setIsMinimized',
    ]) {
      bindings[name] = (value) => changes.push([name, value]);
    }
    const open = await contextCallback(
      'openReply',
      '\n\n  const openNewTopic',
      bindings,
    );
    assert.equal(
      open({
        topicId: 'new-topic',
        topicTitle: 'New',
        parentReplyId: 'parent',
      }),
      failure === null,
    );
    if (failure) {
      assert.deepEqual(
        changes,
        [],
        `failed ${failure} switch must not mutate the draft`,
      );
      assert.equal(bindings.activeDraftRef.current, false);
    } else {
      assert.ok(
        changes.some(
          ([name, value]) => name === 'setTopicId' && value === 'new-topic',
        ),
      );
      assert.ok(
        changes.some(
          ([name, value]) => name === 'setParentReplyId' && value === 'parent',
        ),
      );
      assert.equal(bindings.activeDraftRef.current, true);
    }
  }
});

test('closing a scoped editor cannot discard the latest in-memory draft when persistence fails', async () => {
  for (const scope of [{ tenant: 'shop' }, null]) {
    for (const saved of [false, true]) {
      const changes = [],
        activeDraftRef = { current: true };
      const close = await contextCallback('close', '\n\n  const minimize', {
        isOpen: true,
        scope,
        activeDraftRef,
        persistDraft: () => saved,
        setIsOpen: (value) => changes.push(['open', value]),
        setIsMinimized: (value) => changes.push(['minimized', value]),
      });
      close();
      if (scope && !saved) {
        assert.deepEqual(
          changes,
          [],
          'keep the only fresh copy visible and retryable',
        );
        assert.equal(activeDraftRef.current, true);
      } else {
        assert.deepEqual(changes, [
          ['open', false],
          ['minimized', false],
        ]);
        assert.equal(activeDraftRef.current, false);
      }
    }
  }
});

test('saved drafts reject malformed nested richtext before switching editor/category', async () => {
  const { parseTopicDraft, parseReplyDraft } = draftStorage;
  for (const document of [
    { type: 'doc', content: [null] },
    { type: 'doc', content: [{ type: 'text', text: 'inline root' }] },
    {
      type: 'doc',
      content: [{ type: 'paragraph', content: [{ type: 'not-a-node' }] }],
    },
    {
      type: 'doc',
      content: [
        {
          type: 'paragraph',
          content: [
            {
              type: 'text',
              text: 'unsafe',
              marks: [{ type: 'link', attrs: { href: 'javascript:alert(1)' } }],
            },
          ],
        },
      ],
    },
  ]) {
    const raw = JSON.stringify({
      title: '',
      document,
      quotes: [],
      parentReplyId: null,
    });
    assert.throws(() => parseTopicDraft(raw), /Invalid saved topic draft/);
    assert.throws(
      () => parseReplyDraft(raw, null),
      /Invalid saved topic draft/,
    );
    const scope = { tenant: 'shop', tenantId: tenantA, userId: alice };
    const entries = new Map([
      [getDraftStorageKey(scope, 'topic', null, 'source'), 'outgoing'],
      [getDraftStorageKey(scope, 'topic', null, 'destination'), raw],
    ]);
    const before = new Map(entries);
    assert.throws(
      () =>
        switchTopicDraft(
          {
            getItem: (key) => entries.get(key) ?? null,
            setItem: (key, value) => entries.set(key, value),
            removeItem: (key) => entries.delete(key),
          },
          scope,
          'source',
          'destination',
          {
            title: 'fresh',
            document: { type: 'doc', content: [] },
            quotes: [],
          },
        ),
      /Invalid saved topic draft/,
    );
    assert.deepEqual(entries, before);
  }
});

test('a failed post-success cleanup cannot restore the already-submitted draft in this session', async () => {
  const scope = { tenant: 'shop', tenantId: tenantA, userId: alice };
  const completed = JSON.stringify({
    title: '',
    document: { type: 'doc', content: [{ type: 'paragraph' }] },
    quotes: [],
    parentReplyId: null,
  });
  const pendingDraftCleanupRef = { current: new Set() };
  let blocked = true,
    reads = 0;
  const statuses = [];
  const bindings = {
    scope,
    isCurrentViewer: () => true,
    getDraftStorageKey,
    pendingDraftCleanupRef,
    setDraftStorageStatus: (status) => statuses.push(status),
    localStorage: {
      getItem: () => {
        reads++;
        return completed;
      },
      removeItem: () => {
        if (blocked) throw new Error('storage blocked');
      },
    },
    parseReplyDraft: draftStorage.parseReplyDraft,
    parseTopicDraft: draftStorage.parseTopicDraft,
  };
  bindings.retryDraftCleanup = await contextCallback(
    'retryDraftCleanup',
    '\n\n  // Parse before',
    bindings,
  );
  const clear = await contextCallback(
    'clearDraft',
    '\n\n  const openReply',
    bindings,
  );
  assert.equal(clear('reply', 'topic', null, null), false);
  assert.deepEqual(statuses, ['cleanup_failed']);
  const load = await contextCallback(
    'loadDraft',
    '\n\n  const persistDraft',
    bindings,
  );
  assert.throws(() => load('reply', 'topic', null, null), /storage blocked/);
  assert.equal(
    reads,
    0,
    'never restore a completed payload when cleanup failed',
  );
  blocked = false;
  assert.equal(
    load('reply', 'topic', null, null),
    null,
    'retry cleanup, not restoration',
  );
  assert.equal(pendingDraftCleanupRef.current.size, 0);
  assert.equal(reads, 0);
  assert.equal(
    clear('reply', 'topic', null, null),
    true,
    'explicit cleanup retry never posts again',
  );
  assert.deepEqual(statuses, ['cleanup_failed', 'saved']);
  const composer = readFileSync(
    new URL('../components/composer.tsx', import.meta.url),
    'utf8',
  );
  const closed = composer.slice(
    composer.indexOf('if (!state.isOpen)'),
    composer.indexOf('// Minimized drawer'),
  );
  assert.match(closed, /forum\('draftCleanupFailed'\)/);
  assert.match(closed, /onClick=\{reset\}/);
  assert.match(closed, /forum\('retry'\)/);
});

test('category switching cannot bypass a failed completed-draft cleanup', async () => {
  const scope = { tenant: 'shop', tenantId: tenantA, userId: alice };
  const submitted = {
    title: 'Posted',
    document: { type: 'doc', content: [] },
    quotes: [],
  };
  let reads = 0;
  const changes = [],
    statuses = [];
  const bindings = {
    scope,
    mode: 'topic',
    categoryId: 'source',
    isSubmitting: false,
    isCurrentViewer: () => true,
    title: 'New text',
    document: { type: 'doc', content: [] },
    quotes: [],
    switchTopicDraft,
    retryDraftCleanup: () => {
      throw new Error('storage blocked');
    },
    getDraftStorageKey,
    localStorage: {
      getItem: () => {
        reads++;
        return JSON.stringify(submitted);
      },
      setItem: () => {},
      removeItem: () => {},
    },
    setDraftStorageStatus: (status) => statuses.push(status),
  };
  for (const name of [
    'setTitle',
    'setDocument',
    'setQuotes',
    'setCategoryId',
    'setError',
  ]) {
    bindings[name] = (value) => changes.push([name, value]);
  }
  const select = await contextCallback(
    'selectCategory',
    '\n\n  const close',
    bindings,
  );
  assert.equal(select('destination'), 'failed');
  assert.deepEqual(changes, []);
  assert.deepEqual(statuses, ['failed']);
  assert.equal(reads, 0);
});

for (const [method, end, args] of [
  [
    'openReply',
    '\n\n  const openNewTopic',
    [{ topicId: 'next', topicTitle: 'Next' }],
  ],
  ['openNewTopic', '\n\n  const selectCategory', [{ categoryId: 'next' }]],
  ['selectCategory', '\n\n  const close', ['next']],
  ['appendQuote', '\n\n  const removeQuote', [null, 'late quote']],
  ['removeQuote', '\n\n  const state', [0]],
]) {
  test(`${method}: a same-render pending write freezes context mutations`, async () => {
    const changes = [];
    const bindings = {
      scope: null,
      mode: 'topic',
      categoryId: 'old',
      isOpen: true,
      isSubmitting: false,
      submissionPendingRef: { current: true },
      editorSessionRef: { current: 0 },
      activeDraftRef: { current: true },
      isCurrentViewer: () => true,
      persistDraft: () => true,
      loadDraft: () => null,
      retryDraftCleanup: () => {},
      getDraftStorageKey,
      switchTopicDraft,
      title: 'Current',
      document: { type: 'doc', content: [] },
      quotes: [],
      emptyRichTextDocument: () => ({ type: 'doc', content: [] }),
    };
    for (const name of [
      'setTitle',
      'setDocument',
      'setQuotes',
      'setMode',
      'setTopicId',
      'setTopicTitle',
      'setCategoryId',
      'setParentReplyId',
      'setError',
      'setIsOpen',
      'setIsMinimized',
      'setDraftStorageStatus',
    ]) {
      bindings[name] = (value) => changes.push([name, value]);
    }
    const callback = await contextCallback(method, end, bindings);
    callback(...args);
    assert.deepEqual(
      changes,
      [],
      'the owner lock must work before React renders isSubmitting=true',
    );
    assert.equal(bindings.editorSessionRef.current, 0);
  });
}

for (const [method, end, setter, value] of [
  ['updateTitle', '\n\n  const updateDocument', 'setTitle', 'New title'],
  [
    'updateDocument',
    '\n\n  const clearDraft',
    'setDocument',
    { type: 'doc', content: [] },
  ],
]) {
  test(`${method}: controlled edits require an active, current, non-submitting editor`, async () => {
    for (const blocked of [null, 'pending', 'closed', 'viewer']) {
      const updates = [];
      const callback = await contextCallback(method, end, {
        activeDraftRef: { current: blocked !== 'closed' },
        submissionPendingRef: { current: blocked === 'pending' },
        isCurrentViewer: () => blocked !== 'viewer',
        [setter]: (next) => updates.push(next),
      });
      callback(value);
      assert.deepEqual(
        updates,
        blocked ? [] : [value],
        blocked ?? 'ordinary editing',
      );
    }
  });
}

test('editor lifetime guards survive rejected transitions but not close/reopen, category change or success reset', async () => {
  const sessionUpdates = [];
  const bindings = {
    setEditorSession: (value) => sessionUpdates.push(value),
    submissionPendingRef: { current: false },
    editorSessionRef: { current: 0 },
    activeDraftRef: { current: true },
    isCurrentViewer: () => true,
    isSubmitting: false,
    isOpen: true,
    scope: {},
    persistDraft: () => false,
    loadDraft: () => null,
    retryDraftCleanup: () => {},
    mode: 'topic',
    topicId: 'topic',
    categoryId: 'category',
    parentReplyId: null,
    title: '',
    document: { type: 'doc', content: [] },
    quotes: [],
    getDraftStorageKey,
    switchTopicDraft,
    clearDraft: () => {},
    emptyRichTextDocument: () => ({ type: 'doc', content: [] }),
    setIsSubmitting: () => {},
  };
  for (const name of [
    'setTitle',
    'setDocument',
    'setQuotes',
    'setMode',
    'setTopicId',
    'setTopicTitle',
    'setCategoryId',
    'setParentReplyId',
    'setError',
    'setIsOpen',
    'setIsMinimized',
    'setDraftStorageStatus',
  ]) {
    bindings[name] = () => {};
  }
  const context = readFileSync(
    new URL('./composer-context.tsx', import.meta.url),
    'utf8',
  );
  const value = context.slice(
    context.indexOf('const value = useMemo<ComposerContextValue>'),
  );
  assert.match(value, /setTitle: updateTitle/);
  assert.match(value, /setDocument: updateDocument/);
  assert.match(value, /setSubmitting,/);
  assert.doesNotMatch(value, /setSubmitting: setIsSubmitting/);
  const capture = await contextCallback(
    'captureEditorSession',
    '\n\n  const updateTitle',
    bindings,
  );
  const setSubmitting = await contextCallback(
    'setSubmitting',
    '\n  const [error',
    bindings,
  );
  const current = capture();
  assert.equal(current(), true);
  setSubmitting(true);
  assert.equal(bindings.submissionPendingRef.current, true);
  assert.equal(current(), false, 'the owner lock is synchronous');
  setSubmitting(false);
  assert.equal(
    current(),
    true,
    'a failed write can release its unchanged draft',
  );

  const select = await contextCallback('selectCategory', '\n\n  const close', {
    ...bindings,
    retryDraftCleanup: () => {
      throw new Error('blocked');
    },
  });
  assert.equal(select('category'), 'moved');
  assert.equal(
    current(),
    true,
    'same-category selection does not replace the session',
  );
  assert.equal(select('other'), 'failed');
  assert.equal(
    current(),
    true,
    'a rejected category switch preserves the editor lifetime',
  );
  (await contextCallback('close', '\n\n  const minimize', bindings))();
  assert.equal(
    current(),
    true,
    'failed closing flush keeps the same editor lifetime',
  );
  (await contextCallback('minimize', '\n\n  const expand', bindings))();
  assert.equal(
    current(),
    true,
    'minimizing does not cancel the same editing session',
  );
  (await contextCallback('expand', '\n\n  const reset', bindings))();
  assert.equal(current(), true);

  bindings.scope = null;
  (await contextCallback('close', '\n\n  const minimize', bindings))();
  assert.equal(current(), false);
  bindings.isOpen = false;
  const open = await contextCallback(
    'openReply',
    '\n\n  const openNewTopic',
    bindings,
  );
  assert.equal(open({ topicId: 'topic', topicTitle: 'Same topic' }), true);
  assert.equal(
    current(),
    false,
    'reopening the same target cannot revive the old lifetime',
  );
  const reopened = capture();
  assert.equal(reopened(), true);
  const move = await contextCallback(
    'selectCategory',
    '\n\n  const close',
    bindings,
  );
  assert.equal(move('other'), 'moved');
  assert.equal(reopened(), false);
  const moved = capture();
  assert.equal(moved(), true);
  (await contextCallback('reset', '\n\n  const appendQuote', bindings))();
  assert.equal(
    moved(),
    false,
    'successful post cleanup invalidates outstanding editor work',
  );
  assert.deepEqual(
    sessionUpdates,
    [1, 2, 3, 4],
    'render keys project the same committed editing lifetimes',
  );
});
