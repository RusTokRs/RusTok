import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { compileFunction } from 'node:vm';
import test from 'node:test';
import { findThreadPostIndex } from './thread-navigation.ts';

const source = readFileSync(
  new URL('./use-thread-navigation.ts', import.meta.url),
  'utf8',
);
const executable = stripTypeScriptTypes(
  source
    .replace(/^import[\s\S]*?from ['"][^'"]+['"];$/gm, '')
    .replace(/^export /gm, ''),
);

// Execute the production hook with deterministic hook slots and a small DOM adapter.
// This proves effect ordering/cancellation, not mounted React or browser rendering.
function harness(initial) {
  const states = [],
    refs = [],
    effects = [],
    pendingEffects = [];
  const frames = new Map(),
    listeners = new Map(),
    scrolled = [];
  let stateIndex = 0,
    refIndex = 0,
    effectIndex = 0,
    frameId = 0;
  let props = {
    ...initial,
    replies: Array.from({ length: initial.loadedReplies }, (_, i) => ({
      id: String(i),
    })),
  };
  let posts = [];
  const window = {
    innerHeight: 800,
    requestAnimationFrame: (fn) => {
      frames.set(++frameId, fn);
      return frameId;
    },
    cancelAnimationFrame: (id) => frames.delete(id),
    addEventListener: (type, fn) => {
      if (!listeners.has(type)) listeners.set(type, new Set());
      listeners.get(type).add(fn);
    },
    removeEventListener: (type, fn) => listeners.get(type)?.delete(fn),
  };
  const root = {
    querySelectorAll: () => posts,
    querySelector: (selector) => {
      const n = Number(selector.match(/="(\d+)"/)[1]);
      return posts.find((p) => Number(p.dataset.threadPostNumber) === n);
    },
  };
  const bindings = {
    findThreadPostIndex,
    window,
    ResizeObserver: class {
      observe() {}
      disconnect() {}
    },
    useRef: (value) => {
      const i = refIndex++;
      return (refs[i] ??= { current: value });
    },
    useState: (value) => {
      const i = stateIndex++;
      if (!(i in states)) states[i] = value;
      return [
        states[i],
        (next) => {
          states[i] = typeof next === 'function' ? next(states[i]) : next;
        },
      ];
    },
    useCallback: (fn) => fn,
    useEffect: (fn, deps) => {
      const i = effectIndex++;
      const old = effects[i];
      if (!old || deps.some((d, index) => !Object.is(d, old.deps[index]))) {
        pendingEffects.push(() => {
          old?.cleanup?.();
          effects[i] = { deps, cleanup: fn() };
        });
      }
    },
  };
  const useNavigation = compileFunction(
    `${executable}\nreturn useThreadNavigation;`,
    Object.keys(bindings),
  )(...Object.values(bindings));
  return {
    scrolled,
    frames,
    listeners,
    setPosts(tops) {
      posts = tops.map((top, i) => {
        const post = posts[i] ?? {
          dataset: { threadPostNumber: String(i + 1) },
          getBoundingClientRect() {
            return { top: this.top };
          },
          scrollIntoView: () => scrolled.push(i + 1),
        };
        post.top = top;
        return post;
      });
    },
    render(changes = {}) {
      props = { ...props, ...changes };
      if ('loadedReplies' in changes)
        props.replies = Array.from(
          { length: changes.loadedReplies },
          (_, i) => ({ id: String(i) }),
        );
      stateIndex = refIndex = effectIndex = 0;
      const result = useNavigation(props);
      result.postsRef.current = root;
      return result;
    },
    commit() {
      for (const effect of pendingEffects.splice(0)) effect();
    },
    measure() {
      for (const [id, fn] of [...frames]) {
        frames.delete(id);
        fn();
      }
    },
    scroll() {
      for (const fn of listeners.get('scroll') ?? []) fn();
    },
    unmount() {
      for (const effect of effects) effect.cleanup?.();
    },
  };
}
function deferred() {
  let resolve;
  const promise = new Promise((yes) => {
    resolve = yes;
  });
  return { promise, resolve };
}

test('progress follows posts within its own root and updates on scroll', () => {
  const h = harness({
    scopeKey: 'viewer:topic-a',
    loadedReplies: 2,
    hasMore: false,
  });
  h.setPosts([-900, 100, 900]);
  h.render();
  h.commit();
  h.measure();
  assert.equal(h.render().currentPost, 2);
  h.setPosts([-900, -600, 100]);
  h.scroll();
  h.scroll();
  assert.equal(h.frames.size, 1);
  h.measure();
  assert.equal(h.render().currentPost, 3);
  h.setPosts([100, 900, 2000]);
  h.scroll();
  h.measure();
  assert.equal(h.render().currentPost, 1);
  h.unmount();
  assert.equal(h.frames.size, 0);
  assert.equal(h.listeners.get('scroll').size, 0);
});

test('last loads before scrolling, blocks same-tick duplicate clicks, and waits for the DOM commit', async () => {
  const gate = deferred();
  let calls = 0;
  const h = harness({
    scopeKey: 'a',
    loadedReplies: 1,
    hasMore: true,
    loadReplies: (target) => {
      assert.equal(target, 'last');
      calls++;
      return gate.promise;
    },
  });
  h.setPosts([0, 300]);
  let nav = h.render();
  h.commit();
  const pending = nav.jumpToPost('last');
  await nav.jumpToPost('last');
  assert.equal(calls, 1);
  assert.equal(h.render().isNavigating, true);
  gate.resolve({ post: 4, complete: true });
  await pending;
  assert.deepEqual(h.scrolled, []);
  h.setPosts([0, 300, 600, 900]);
  nav = h.render({ loadedReplies: 3, hasMore: false });
  h.commit();
  assert.deepEqual(h.scrolled, [4]);
  assert.equal(nav.isNavigating, false);
});

test('a bounded partial jump remains visibly incomplete, not a false end', async () => {
  const h = harness({
    scopeKey: 'a',
    loadedReplies: 0,
    hasMore: true,
    loadReplies: async () => ({ post: 3, complete: false }),
  });
  h.setPosts([0]);
  let nav = h.render();
  h.commit();
  await nav.jumpToPost('last');
  h.setPosts([0, 300, 600]);
  nav = h.render({ loadedReplies: 2 });
  h.commit();
  assert.equal(nav.navigationIncomplete, true);
  assert.deepEqual(h.scrolled, [3]);
  assert.equal(h.render({ hasMore: false }).navigationIncomplete, false);
});

test('scope changes and unmount invalidate old completions without unlocking a newer jump', async () => {
  const old = deferred(),
    newer = deferred();
  const h = harness({
    scopeKey: 'old',
    loadedReplies: 0,
    hasMore: true,
    loadReplies: () => old.promise,
  });
  h.setPosts([0]);
  let nav = h.render();
  h.commit();
  const before = nav.jumpToPost('last');
  nav = h.render({ scopeKey: 'new', loadReplies: () => newer.promise });
  h.commit();
  const after = nav.jumpToPost('last');
  old.resolve({ post: 9, complete: false });
  await before;
  assert.equal(h.render().isNavigating, true);
  assert.deepEqual(h.scrolled, []);
  h.unmount();
  newer.resolve({ post: 10, complete: true });
  await after;
  assert.deepEqual(h.scrolled, []);
});

test('beginning and an already loaded end need no fetch; invalid positions are ignored', async () => {
  const h = harness({
    scopeKey: 'a',
    loadedReplies: 1,
    hasMore: false,
    loadReplies: () => assert.fail('already loaded'),
  });
  h.setPosts([0, 300]);
  let nav = h.render();
  h.commit();
  await nav.jumpToPost(1);
  h.render();
  h.commit();
  await nav.jumpToPost('last');
  h.render();
  h.commit();
  await nav.jumpToPost(-2);
  assert.deepEqual(h.scrolled, [1, 2]);
});

test('position clamps immediately when a refresh replaces a long prefix', () => {
  const h = harness({ scopeKey: 'a', loadedReplies: 2, hasMore: false });
  h.setPosts([-900, -600, 100]);
  h.render();
  h.commit();
  h.measure();
  assert.equal(h.render().currentPost, 3);
  assert.equal(h.render({ loadedReplies: 0 }).currentPost, 1);
  assert.equal(h.render({ scopeKey: 'b' }).currentPost, 1);
});

test('ordinary append after a completed jump does not scroll back to the old target', async () => {
  const h = harness({ scopeKey: 'a', loadedReplies: 1, hasMore: true });
  h.setPosts([0, 300]);
  const nav = h.render();
  h.commit();
  await nav.jumpToPost(2);
  h.render();
  h.commit();
  assert.deepEqual(h.scrolled, [2]);
  h.setPosts([0, 300, 600]);
  h.render({ loadedReplies: 2 });
  h.commit();
  assert.deepEqual(h.scrolled, [2]);
});

test('a same-length refreshed reply snapshot is measured from its replacement DOM nodes', () => {
  const h = harness({ scopeKey: 'a', loadedReplies: 2, hasMore: false });
  h.setPosts([0, 300, 600]);
  h.render();
  h.commit();
  h.measure();
  assert.equal(h.render().currentPost, 1);
  h.setPosts([]);
  h.setPosts([-900, -600, 100]);
  h.render({ loadedReplies: 2 });
  h.commit();
  h.measure();
  assert.equal(h.render().currentPost, 3);
});
