import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { compileFunction } from 'node:vm';
import test from 'node:test';
import { loadThreadReplyPages, threadJumpResult } from './thread-navigation.ts';
import { isForumReplyId } from './reply-anchor.ts';

const reply = (id) => ({ id, authorId: null });
function deferred() {
  let resolve;
  const promise = new Promise((yes) => {
    resolve = yes;
  });
  return { promise, resolve };
}
function harness(surface, transport) {
  const source = readFileSync(
    new URL(`./${surface}.tsx`, import.meta.url),
    'utf8',
  );
  const start = source.indexOf('  const loadMoreReplies = useCallback(');
  const finish = source.indexOf('\n\n  useEffect', start);
  assert.ok(start >= 0 && finish > start);
  let page = { key: 'scope', replies: [reply('first')], nextCursor: 'a' };
  const errors = [],
    busy = [];
  const request = { current: 0 },
    key = { current: 'scope' };
  const bindings = {
    auth: {},
    selectedTopicId: 'topic',
    topicId: 'topic',
    replies: page.replies,
    repliesCursor: 'a',
    tenantId: 'tenant-id',
    tenantSlug: 'tenant-slug',
    token: 'signed',
    locale: 'ru',
    contentKey: 'scope',
    detailKey: 'scope',
    contentKeyRef: key,
    detailKeyRef: key,
    replyRequest: request,
    repliesRequest: request,
    repliesPendingRef: { current: false },
    loadThreadReplyPages,
    threadJumpResult,
    isForumReplyId,
    focusReplyId: null,
    memberCards: [],
    fetchStorefrontReplies: transport,
    setLoadingReplies: (value) => busy.push(value),
    setContent: (fn) => {
      page = fn(page);
    },
    setDetailPage: (fn) => {
      page = fn(page);
    },
    setContentError: (value) => errors.push(value),
    setDetailsError: (value) => errors.push(value),
    setAnchorMissing: () => assert.fail('no anchor'),
    loadMemberCards: async () => {},
    forum: (id) => id,
  };
  const load = compileFunction(
    `${stripTypeScriptTypes(source.slice(start, finish))}\nreturn loadMoreReplies;`,
    ['useCallback', ...Object.keys(bindings)],
  )((fn) => fn, ...Object.values(bindings));
  let unmount;
  const lifecycle = source.match(
    /  useEffect\(\(\) => \(\) => \{[\s\S]*?\n  }, \[\]\);/,
  );
  assert.ok(lifecycle, 'paging must cancel on surface unmount');
  compileFunction(lifecycle[0], ['useEffect', ...Object.keys(bindings)])(
    (setup) => {
      unmount = setup();
    },
    ...Object.values(bindings),
  );
  return {
    load,
    bindings,
    request,
    key,
    errors,
    busy,
    unmount: () => unmount(),
    page: () => page,
  };
}

for (const surface of ['forum-section', 'topic-detail-view']) {
  test(`${surface}: end pages through the same scoped transport and commits before returning the target`, async () => {
    const calls = [];
    const h = harness(surface, async (options) => {
      calls.push(options);
      return options.after === 'a'
        ? { items: [reply('second')], nextCursor: 'b' }
        : { items: [reply('third')], nextCursor: null };
    });
    assert.deepEqual(await h.load('last'), { post: 4, complete: true });
    assert.deepEqual(
      calls,
      ['a', 'b'].map((after) => ({
        tenantId: 'tenant-id',
        tenantSlug: 'tenant-slug',
        token: 'signed',
        topicId: 'topic',
        locale: 'ru',
        after,
      })),
    );
    assert.deepEqual(h.page().replies, [
      reply('first'),
      reply('second'),
      reply('third'),
    ]);
    assert.equal(h.page().nextCursor, null);
    assert.deepEqual(h.busy, [true, false]);
  });

  test(`${surface}: same-tick duplicate reads are blocked and an old finally cannot unlock a refreshed snapshot`, async () => {
    const old = deferred(),
      newer = deferred();
    let calls = 0;
    const h = harness(surface, () =>
      ++calls === 1 ? old.promise : newer.promise,
    );
    const before = h.load('last');
    assert.equal(await h.load('last'), null);
    assert.equal(calls, 1);
    // The production first-page refresh invalidates its old page request and lock.
    ++h.request.current;
    h.bindings.repliesPendingRef.current = false;
    const after = h.load('last');
    old.resolve({ items: [reply('old')], nextCursor: null });
    await before;
    assert.equal(h.bindings.repliesPendingRef.current, true);
    assert.deepEqual(h.page().replies, [reply('first')]);
    newer.resolve({ items: [reply('new')], nextCursor: null });
    await after;
    assert.equal(h.bindings.repliesPendingRef.current, false);
    assert.deepEqual(h.page().replies, [reply('first'), reply('new')]);
  });

  test(`${surface}: an account/tenant/topic switch stops further fetches and stale writes`, async () => {
    const gate = deferred();
    let calls = 0;
    const h = harness(surface, () => {
      calls++;
      return gate.promise;
    });
    const pending = h.load('last');
    h.key.current = 'new-scope';
    gate.resolve({ items: [reply('private-to-old-viewer')], nextCursor: 'b' });
    assert.equal(await pending, null);
    assert.equal(calls, 1);
    assert.deepEqual(h.page().replies, [reply('first')]);
    assert.deepEqual(h.errors, []);
    const unmounted = deferred();
    let unmountedCalls = 0;
    const gone = harness(surface, () => {
      unmountedCalls++;
      return unmounted.promise;
    });
    const oldRead = gone.load('last');
    gone.unmount();
    unmounted.resolve({ items: [reply('late')], nextCursor: 'more' });
    assert.equal(await oldRead, null);
    assert.equal(unmountedCalls, 1);
    assert.deepEqual(gone.page().replies, [reply('first')]);
  });

  test(`${surface}: network failure keeps the existing prefix and allows retry`, async () => {
    let calls = 0;
    const h = harness(surface, async () => {
      if (++calls === 1) throw new Error('offline');
      return { items: [reply('second')], nextCursor: null };
    });
    assert.equal(await h.load('last'), null);
    assert.deepEqual(h.page().replies, [reply('first')]);
    assert.deepEqual(h.errors, ['errorLoad']);
    assert.equal(h.bindings.repliesPendingRef.current, false);
    assert.deepEqual(await h.load('last'), { post: 3, complete: true });
  });
}

test('both hosts wire scoped DOM positions, bounded navigation, and a localized partial-result notice', () => {
  for (const surface of ['thread-panel', 'topic-detail-view']) {
    const source = readFileSync(
      new URL(`./${surface}.tsx`, import.meta.url),
      'utf8',
    );
    assert.match(source, /ref=\{navigation\.postsRef\}/);
    assert.match(source, /currentPost=\{navigation\.currentPost\}/);
    assert.match(source, /onJumpToPost=\{navigation\.jumpToPost\}/);
    assert.match(source, /data-thread-post-number=\{1\}/);
    assert.match(source, /data-thread-post-number=\{(?:index|idx) \+ 2\}/);
    assert.match(source, /forum\('morePostsToEnd'\)/);
    assert.doesNotMatch(
      source,
      /document\.getElementById|new IntersectionObserver|currentPost=\{1\}/,
    );
  }
  const timeline = readFileSync(
    new URL('./timeline-scroller.tsx', import.meta.url),
    'utf8',
  );
  assert.match(timeline, /onJumpToPost\?\.\('last'\)/);
  assert.match(timeline, /role="progressbar"/);
  assert.match(timeline, /disabled=\{isNavigating \|\| !onJumpToPost\}/);
});
