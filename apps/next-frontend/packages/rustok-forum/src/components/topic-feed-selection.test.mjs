import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { compileFunction } from 'node:vm';
import test from 'node:test';
import { topicFeedSelection } from './topic-feed-selection.ts';

const source = readFileSync(
  new URL('./forum-section.tsx', import.meta.url),
  'utf8',
);
const start = source.indexOf('  const loadTopics = useCallback(');
const finish = source.indexOf('  const loadTopicDetails =', start);
function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
function harness(transport) {
  let page = {
    key: 'old-feed',
    items: [{ id: 'already-loaded' }],
    nextCursor: 'old-cursor',
  };
  const calls = [],
    errors = [],
    busy = [];
  const bindings = {
    auth: {},
    activeTab: 'latest',
    searchQuery: ' needle ',
    token: 'signed',
    tenantId: 'tenant',
    tenantSlug: 'shop',
    locale: 'ru',
    selectedCategoryId: 'category',
    topicsRequest: { current: 0 },
    feedKeyRef: { current: 'old-feed' },
    feedKey: 'old-feed',
    topicFeedSelection,
    useCallback: (fn) => fn,
    setTopicPage: (value) => {
      page = typeof value === 'function' ? value(page) : value;
    },
    setTopicsError: (value) => errors.push(value),
    setLoadingTopics: (value) => busy.push(value),
    forum: (id) => id,
    loadMemberCards: async () => {},
    fetchStorefrontTopics: (options) => {
      calls.push(['audience', options]);
      return transport(options);
    },
    fetchStorefrontUnreadTopics: (options) => {
      calls.push(['unread', options]);
      return transport(options);
    },
  };
  const render = () => {
    bindings.normalizedSearchQuery = bindings.searchQuery.trim();
    return compileFunction(
      `${stripTypeScriptTypes(source.slice(start, finish))}\nreturn loadTopics;`,
      Object.keys(bindings),
    )(...Object.values(bindings));
  };
  return { bindings, calls, errors, busy, render, page: () => page };
}

test('tab selection maps to owner fields; unread remains an owner read-state query', () => {
  for (const tab of ['latest', 'top', 'unread', 'solved']) {
    assert.deepEqual(topicFeedSelection(tab, '  Тема  '), {
      search: 'Тема',
      order: tab === 'top' ? 'TOP' : 'LATEST',
      solvedOnly: tab === 'solved',
    });
    assert.equal(topicFeedSelection(tab, ' \n ').search, undefined);
  }
});

for (const tab of ['latest', 'top', 'unread', 'solved']) {
  test(`${tab}: first and continuation pages share selection and preserve owner order`, async () => {
    const h = harness(async ({ after }) => ({
      items: after ? [{ id: 'second' }] : [{ id: 'first' }],
      nextCursor: after ? null : 'next',
    }));
    h.bindings.activeTab = tab;
    await h.render()();
    assert.deepEqual(
      h.page().items.map((item) => item.id),
      ['first'],
    );
    await h.render()(h.page().nextCursor);
    assert.deepEqual(
      h.page().items.map((item) => item.id),
      ['first', 'second'],
    );
    assert.equal(h.page().nextCursor, null);
    assert.equal(h.calls[0][0], tab === 'unread' ? 'unread' : 'audience');
    const [first, second] = h.calls.map((call) => call[1]);
    assert.deepEqual({ ...first, after: 'next' }, second);
    assert.equal(first.search, 'needle');
    assert.equal(first.order, tab === 'top' ? 'TOP' : 'LATEST');
    assert.equal(first.solvedOnly, tab === 'solved');
    assert.equal(second.token, 'signed');
  });
}

test('search changes hide the old feed by key and discard both late success and late errors', async () => {
  for (const fail of [false, true]) {
    const old = deferred();
    const h = harness(({ search }) =>
      search === 'needle'
        ? old.promise
        : Promise.resolve({
            items: [{ id: 'fresh-match' }],
            nextCursor: null,
          }),
    );
    const pending = h.render()('old-cursor');
    h.bindings.searchQuery = 'new-query';
    h.bindings.feedKey = h.bindings.feedKeyRef.current = 'new-feed';
    await h.render()();
    const errors = [...h.errors],
      busy = [...h.busy];
    if (fail) old.reject(new Error('old request failed'));
    else
      old.resolve({
        items: [{ id: 'wrong-selection' }],
        nextCursor: 'wrong-cursor',
      });
    await pending;
    assert.equal(h.page().key, 'new-feed');
    assert.deepEqual(
      h.page().items.map((item) => item.id),
      ['fresh-match'],
    );
    assert.deepEqual(h.errors, errors);
    assert.deepEqual(h.busy, busy);
  }
});

test('a render invalidates an old result before the replacement request starts', async () => {
  const pending = deferred();
  const h = harness(() => pending.promise);
  const load = h.render()();
  h.bindings.feedKeyRef.current = 'changed-between-render-and-effect';
  pending.resolve({ items: [{ id: 'stale' }], nextCursor: null });
  await load;
  assert.deepEqual(
    h.page().items.map((item) => item.id),
    ['already-loaded'],
  );
});

test('anonymous unread cannot fall back to the public topic list', async () => {
  const h = harness(() =>
    assert.fail('no owner request without read-state auth'),
  );
  h.bindings.activeTab = 'unread';
  h.bindings.token = null;
  await h.render()();
  assert.deepEqual(h.page().items, []);
  assert.deepEqual(h.calls, []);
});

test('owner failure is explicit and does not turn a selected page into empty success', async () => {
  const h = harness(async () => {
    throw new Error('owner unavailable');
  });
  await h.render()();
  assert.equal(h.errors.at(-1), 'errorLoad');
  assert.equal(h.busy.at(-1), false);
  assert.deepEqual(
    h.page().items.map((item) => item.id),
    ['already-loaded'],
  );
});

test('selection invalidates the page key and search is debounced without local filtering or sorting', () => {
  const feed = readFileSync(
    new URL('./topic-feed.tsx', import.meta.url),
    'utf8',
  );
  assert.match(
    source,
    /JSON\.stringify\(\[viewerKey, selectedCategoryId, activeTab, normalizedSearchQuery\]\)/,
  );
  assert.match(
    source,
    /topicPage\?\.key === feedKey \? topicPage\.items : \[\]/,
  );
  assert.match(source, /clearTimeout\(timer\)/);
  assert.match(source, /normalizedSearchQuery \? 250 : 0/);
  assert.match(feed, /topics\.map\(/);
  assert.doesNotMatch(
    feed,
    /topics\.filter\(|\.sort\(|filteredTopics|filtersLoadedOnly/,
  );
  assert.match(feed, /aria-label=\{forum\('clearSearch'\)\}/);
});

test('a stale refresh callback cannot invalidate an in-flight page of the current selection', async () => {
  const fresh = deferred();
  const h = harness(({ search }) =>
    search === 'new-query'
      ? fresh.promise
      : Promise.resolve({
          items: [{ id: 'stale' }],
          nextCursor: null,
        }),
  );
  const staleRefresh = h.render(); // e.g. captured before awaiting a read receipt/post.
  h.bindings.searchQuery = 'new-query';
  h.bindings.feedKey = h.bindings.feedKeyRef.current = 'new-feed';
  const pending = h.render()();
  await staleRefresh();
  assert.equal(
    h.calls.length,
    1,
    'the old callback must not start another request/generation',
  );
  fresh.resolve({ items: [{ id: 'current-match' }], nextCursor: null });
  await pending;
  assert.equal(h.page().key, 'new-feed');
  assert.deepEqual(
    h.page().items.map(({ id }) => id),
    ['current-match'],
  );
  assert.equal(h.busy.at(-1), false);
});

test('editing only surrounding whitespace preserves the request callback and appended pages', async () => {
  const h = harness(async () => ({
    items: [{ id: 'first' }],
    nextCursor: 'next',
  }));
  let memo;
  h.bindings.useCallback = (fn, dependencies) => {
    if (
      !memo ||
      dependencies.some((item, i) => !Object.is(item, memo.dependencies[i]))
    ) {
      memo = { fn, dependencies };
    }
    return memo.fn;
  };
  const first = h.render();
  await first();
  h.bindings.searchQuery = '   needle   ';
  assert.equal(
    h.render(),
    first,
    'equivalent input must not schedule a first-page replacement',
  );
  assert.equal(h.page().nextCursor, 'next');
});

test('a delayed post callback cannot cancel detail/paging requests of another selected thread', async () => {
  const detailsRequest = { current: 4 },
    repliesRequest = { current: 7 };
  const repliesPendingRef = { current: true };
  const bindings = {
    auth: {},
    tenantId: 'tenant',
    tenantSlug: 'shop',
    token: 'signed',
    locale: 'en',
    viewerKey: 'viewer',
    detailKeyRef: { current: 'viewer:new-topic' },
    detailsRequest,
    repliesRequest,
    repliesPendingRef,
    loadMemberCards: async () => {},
    forum: (id) => id,
    setLoadingReplies: () =>
      assert.fail('must not reset the new thread loading state'),
  };
  const begin = source.indexOf('  const loadTopicDetails = useCallback(');
  const end = source.indexOf('  const loadMoreReplies =', begin);
  const load = compileFunction(
    `${stripTypeScriptTypes(source.slice(begin, end))}\nreturn loadTopicDetails;`,
    ['useCallback', ...Object.keys(bindings)],
  )((fn) => fn, ...Object.values(bindings));
  await load('old-topic');
  assert.equal(detailsRequest.current, 4);
  assert.equal(repliesRequest.current, 7);
  assert.equal(repliesPendingRef.current, true);
});
