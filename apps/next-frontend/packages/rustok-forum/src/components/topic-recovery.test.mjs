import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { compileFunction } from 'node:vm';
import test from 'node:test';

const source = readFileSync(
  new URL('./topic-detail-view.tsx', import.meta.url),
  'utf8',
);

// Run the production callback with its transport and React ref/state bindings
// injected, rather than a second implementation of the retry rules.
function callback(name, end, bindings) {
  const start = source.indexOf(`  const ${name} = useCallback(`);
  assert.notEqual(start, -1);
  const finish = source.indexOf(end, start);
  assert.notEqual(finish, -1);
  const body = stripTypeScriptTypes(source.slice(start, finish));
  return compileFunction(`${body}\nreturn ${name};`, [
    'useCallback',
    ...Object.keys(bindings),
  ])((fn) => fn, ...Object.values(bindings));
}

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

function readBindings(transport) {
  const errors = [];
  return {
    token: 'token',
    topic: { id: 'topic-a' },
    loading: false,
    tenantId: 'tenant-id',
    tenantSlug: 'tenant',
    locale: 'ru',
    contentKey: 'viewer-a',
    contentKeyRef: { current: 'viewer-a' },
    readAttemptRef: { current: null },
    markForumTopicRead: transport,
    setReadError: (value) => errors.push(value),
    forum: (key) => key,
    errors,
  };
}

const readCallback = (bindings) =>
  callback(
    'markTopicRead',
    '\n\n  useEffect(() => { void markTopicRead();',
    bindings,
  );
const categoriesCallback = (bindings) =>
  callback(
    'loadCategories',
    '\n\n  useEffect(() => {\n    void loadCategories();',
    bindings,
  );

test('a failed receipt retries only on demand, then succeeds without duplicate writes', async () => {
  let calls = 0;
  const bindings = readBindings(async (options) => {
    assert.equal(options.token, 'token');
    assert.equal(options.tenantId, 'tenant-id');
    assert.equal(options.topicId, 'topic-a');
    if (++calls === 1) throw new Error('offline');
  });
  const markRead = readCallback(bindings);
  await markRead();
  assert.deepEqual(bindings.errors, ['errorMarkRead']);
  await markRead(); // Effect rerender is not an implicit retry loop.
  assert.equal(calls, 1);
  await markRead(true);
  assert.equal(calls, 2);
  assert.deepEqual(bindings.errors, ['errorMarkRead', null]);
  await markRead(true);
  await markRead();
  assert.equal(calls, 2);
});

test('parallel retry clicks do not duplicate a pending read receipt', async () => {
  const gate = deferred();
  let calls = 0;
  const bindings = readBindings(() => {
    calls++;
    return gate.promise;
  });
  const markRead = readCallback(bindings);
  const pending = markRead();
  await markRead(true);
  await markRead();
  assert.equal(calls, 1);
  gate.resolve();
  await pending;
  assert.equal(bindings.readAttemptRef.current.status, 'succeeded');
});

test('old viewer completions cannot overwrite the new receipt or disclose its error', async () => {
  const gate = deferred();
  const bindings = readBindings(() => gate.promise);
  const pending = readCallback(bindings)();
  const next = { key: 'new-viewer', status: 'pending' };
  bindings.readAttemptRef.current = next;
  bindings.contentKeyRef.current = 'viewer-b';
  gate.reject(new Error('old viewer denied'));
  await pending;
  assert.equal(bindings.readAttemptRef.current, next);
  assert.equal(next.status, 'pending');
  assert.deepEqual(bindings.errors, []);
});

test('explicit receipt retry still works while a thread refresh is loading', async () => {
  let calls = 0;
  const bindings = readBindings(async () => {
    calls++;
  });
  bindings.loading = true;
  const markRead = readCallback(bindings);
  await markRead();
  assert.equal(calls, 0);
  await markRead(true);
  assert.equal(calls, 1);
});

test('anonymous or missing-topic views never create a read receipt', async () => {
  for (const changes of [{ token: '' }, { topic: null }]) {
    const bindings = {
      ...readBindings(async () => assert.fail('unexpected write')),
      ...changes,
    };
    await readCallback(bindings)(true);
  }
});

function categoryBindings(transport) {
  const pages = [];
  const errors = [];
  return {
    auth: { token: 'token' },
    tenantId: 'tenant-id',
    tenantSlug: 'tenant',
    token: 'token',
    locale: 'ru',
    categoryKey: 'viewer-a',
    categoryKeyRef: { current: 'viewer-a' },
    categoryRequest: { current: 0 },
    fetchAllStorefrontCategories: transport,
    setCategoryPage: (page) => pages.push(page),
    setCategoriesError: (error) => errors.push(error),
    forum: (key) => key,
    pages,
    errors,
  };
}

test('category failure can be retried through the same tenant-scoped owner read', async () => {
  let calls = 0;
  const bindings = categoryBindings(async (options) => {
    assert.deepEqual(options, {
      tenantId: 'tenant-id',
      tenantSlug: 'tenant',
      token: 'token',
      locale: 'ru',
    });
    if (++calls === 1) throw new Error('offline');
    return { items: [{ id: 'category-a' }] };
  });
  const load = categoriesCallback(bindings);
  await load();
  assert.deepEqual(bindings.errors, ['errorLoad']);
  await load();
  assert.deepEqual(bindings.pages, [
    { key: 'viewer-a', items: [{ id: 'category-a' }] },
  ]);
  assert.deepEqual(bindings.errors, ['errorLoad', null]);
});

test('a late category response cannot roll back a newer retry or cross a viewer switch', async () => {
  for (const oldFails of [false, true]) {
    const first = deferred();
    let calls = 0;
    const bindings = categoryBindings(() =>
      ++calls === 1
        ? first.promise
        : Promise.resolve({ items: [{ id: 'new' }] }),
    );
    const load = categoriesCallback(bindings);
    const old = load();
    await load();
    if (oldFails) first.reject(new Error('old failure'));
    else first.resolve({ items: [{ id: 'old' }] });
    await old;
    assert.deepEqual(
      bindings.pages.map((p) => p.items),
      [[{ id: 'new' }]],
    );
    assert.deepEqual(bindings.errors, [null]);
  }
  const gate = deferred();
  const bindings = categoryBindings(() => gate.promise);
  const pending = categoriesCallback(bindings)();
  bindings.categoryKeyRef.current = 'viewer-b';
  gate.resolve({ items: [{ id: 'private-to-a' }] });
  await pending;
  assert.deepEqual(bindings.pages, []);
});

test('both retry controls repeat content, categories and failed read receipts', () => {
  const calls = [];
  const retry = callback('retryLoad', '\n\n  const navigation =', {
    loadData: () => calls.push('content'),
    loadCategories: () => calls.push('categories'),
    markTopicRead: (force) => calls.push(['receipt', force]),
  });
  retry();
  assert.deepEqual(calls, ['content', 'categories', ['receipt', true]]);
  assert.equal(source.match(/onClick=\{retryLoad\}/g)?.length, 2);
});
