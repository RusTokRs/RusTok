import assert from 'node:assert/strict';
import test from 'node:test';
import {
  findThreadPostIndex,
  loadThreadReplyPages,
  MAX_THREAD_NAVIGATION_PAGES,
  threadJumpResult,
  threadPostTotal,
} from './thread-navigation.ts';

const reply = (id) => ({ id: String(id) });
const current = () => true;

test('end follows opaque owner cursors to exhaustion instead of guessing from a count', async () => {
  const calls = [];
  const page = await loadThreadReplyPages({
    items: [reply(1)],
    nextCursor: 'opaque-a',
    target: 'last',
    isCurrent: current,
    loadPage: async (after) => {
      calls.push(after);
      return after === 'opaque-a'
        ? { items: [reply(2)], nextCursor: 'opaque-b' }
        : { items: [reply(3)], nextCursor: null };
    },
  });
  assert.deepEqual(calls, ['opaque-a', 'opaque-b']);
  assert.deepEqual(page.items, [reply(1), reply(2), reply(3)]);
  assert.deepEqual(threadJumpResult(page, 'last'), { post: 4, complete: true });
});

test('bounded end navigation reports remaining pages and resumes from the retained cursor', async () => {
  let calls = 0;
  const loadPage = async (after) => {
    calls++;
    const n = Number(after);
    return { items: [reply(n)], nextCursor: n === 25 ? null : String(n + 1) };
  };
  const first = await loadThreadReplyPages({
    items: [],
    nextCursor: '1',
    target: 'last',
    isCurrent: current,
    loadPage,
  });
  assert.equal(calls, MAX_THREAD_NAVIGATION_PAGES);
  assert.equal(first.nextCursor, '21');
  assert.deepEqual(threadJumpResult(first, 'last'), {
    post: 21,
    complete: false,
  });
  const second = await loadThreadReplyPages({
    ...first,
    target: 'last',
    isCurrent: current,
    loadPage,
  });
  assert.equal(calls, 25);
  assert.equal(second.items.length, 25);
  assert.deepEqual(threadJumpResult(second, 'last'), {
    post: 26,
    complete: true,
  });
});

test('overlapping pages preserve unique post numbers and owner order', async () => {
  const page = await loadThreadReplyPages({
    items: [reply(1), reply(2)],
    nextCursor: 'a',
    target: 'last',
    isCurrent: current,
    loadPage: async () => ({
      items: [{ id: '2', updated: true }, reply(3)],
      nextCursor: null,
    }),
  });
  assert.deepEqual(page.items, [
    reply(1),
    { id: '2', updated: true },
    reply(3),
  ]);
  assert.equal(threadJumpResult(page, 'last').post, 4);
});

test('numbered jumps stop when the post is loaded; ordinary load-more reads exactly one page', async () => {
  for (const target of [3, undefined]) {
    let calls = 0;
    const result = await loadThreadReplyPages({
      items: [reply(1)],
      nextCursor: 'a',
      target,
      isCurrent: current,
      loadPage: async () => {
        calls++;
        return { items: [reply(2)], nextCursor: 'b' };
      },
    });
    assert.equal(calls, 1);
    assert.equal(result.nextCursor, 'b');
  }
  const result = await loadThreadReplyPages({
    items: [reply(1)],
    nextCursor: 'a',
    target: 1,
    isCurrent: current,
    loadPage: () => assert.fail('opening post is already loaded'),
  });
  assert.equal(result.items.length, 1);
});

test('a terminal cursor needs no request, including a thread with no replies', async () => {
  const result = await loadThreadReplyPages({
    items: [],
    nextCursor: null,
    target: 'last',
    isCurrent: current,
    loadPage: () => assert.fail('terminal read'),
  });
  assert.deepEqual(threadJumpResult(result, 'last'), {
    post: 1,
    complete: true,
  });
});

test('invalid post numbers cannot trigger owner reads', async () => {
  for (const target of [0, -1, 1.5, NaN, Infinity]) {
    await assert.rejects(
      loadThreadReplyPages({
        items: [],
        nextCursor: 'a',
        target,
        isCurrent: current,
        loadPage: () => assert.fail('invalid request'),
      }),
      /Invalid thread post/,
    );
  }
});

test('cursor cycles and transport errors reject instead of claiming the end was reached', async () => {
  for (const cycle of [false, true]) {
    const cursors = [];
    await assert.rejects(
      loadThreadReplyPages({
        items: [],
        nextCursor: 'a',
        target: 'last',
        isCurrent: current,
        loadPage: async (after) => {
          cursors.push(after);
          return { items: [], nextCursor: cycle && after === 'a' ? 'b' : 'a' };
        },
      }),
      /cursor did not advance/,
    );
    assert.equal(cursors.length, cycle ? 2 : 1);
  }
  await assert.rejects(
    loadThreadReplyPages({
      items: [],
      nextCursor: 'a',
      target: 'last',
      isCurrent: current,
      loadPage: async () => {
        throw new Error('offline');
      },
    }),
    /offline/,
  );
});

test('viewer changes or refresh invalidation stop the walk before the next page', async () => {
  let active = false;
  let calls = 0;
  const options = {
    items: [],
    nextCursor: 'a',
    target: 'last',
    isCurrent: () => active,
    loadPage: async () => {
      calls++;
      active = false;
      return { items: [reply(1)], nextCursor: 'b' };
    },
  };
  assert.equal(await loadThreadReplyPages(options), null);
  assert.equal(calls, 0);
  active = true;
  assert.equal(await loadThreadReplyPages(options), null);
  assert.equal(calls, 1);
});

test('visible totals become exact on cursor exhaustion and never trail the loaded prefix', () => {
  assert.equal(threadPostTotal(12, 900, false), 13);
  assert.equal(threadPostTotal(12, 0, true), 14);
  assert.equal(threadPostTotal(12, 900, true), 901);
  assert.equal(threadPostTotal(0, 0, false), 1);
});

test('reading position handles tall posts, scrolling both ways, and logarithmic DOM measurements', () => {
  const tops = [-1000, 3000, 3100, 9000];
  for (const [line, expected] of [
    [-2000, 0],
    [200, 0],
    [3000, 1],
    [4000, 2],
    [10000, 3],
  ]) {
    assert.equal(
      findThreadPostIndex(tops.length, (i) => tops[i], line),
      expected,
    );
  }
  assert.equal(
    findThreadPostIndex(0, () => assert.fail(), 200),
    -1,
  );
  let reads = 0;
  assert.equal(
    findThreadPostIndex(
      10000,
      (i) => {
        reads++;
        return i * 100;
      },
      543250,
    ),
    5432,
  );
  assert.ok(reads <= 15);
});
