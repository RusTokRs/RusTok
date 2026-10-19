/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import { ProductWriteAttempt } from './write-attempt.ts';
import {
  ProductWriteJournal,
  browserProductWriteJournal,
  PRODUCT_EDITOR_INTENT_QUERY_KEY,
  isProductEditorIntent
} from './write-journal.ts';
import { memoryJournal, testScope, testIntent } from './write-test-support.mjs';

const unknown = { status: 'unknown', message: 'Lost response' };
const state = () => ({ text: null, locked: false });
const save = () => ({
  scope: testScope,
  isNew: true,
  translations: [{ locale: 'en', title: 'Chair' }],
  revision: null,
  tags: [],
  attributePatches: [{ attributeId: 'material', kind: 'TEXT', text: 'cotton' }],
  activeLocale: 'en'
});
const requestFor = (operation) =>
  operation === 'save'
    ? save()
    : {
        scope: testScope,
        id: 'product',
        ...(operation === 'status' ? { status: 'ACTIVE', revision: 7 } : {})
      };
const confirmation = (operation) => ({
  status: 'confirmed',
  value: {
    id: operation === 'save' ? 'created' : 'product',
    revision: operation === 'delete' ? null : 8
  }
});

for (const operation of ['save', 'status', 'delete']) {
  test(`${operation}: reload resumes the persisted snapshot and caller key, then scrubs the body`, async () => {
    const storage = state();
    const resource = operation === 'save' ? 'new' : 'product';
    const first = new ProductWriteAttempt(memoryJournal(resource, storage));
    let initial, durable;
    await first.execute(
      operation,
      requestFor(operation),
      async (request, key) => {
        initial = { request, key };
        durable = JSON.parse(storage.text).record;
        return unknown;
      }
    );
    assert.equal(durable.phase, 'pending');
    assert.equal(durable.attempt.key, initial.key);
    assert.deepEqual(durable.attempt.request, initial.request);
    assert.equal(first.pendingOperation, operation);
    const reloaded = new ProductWriteAttempt(memoryJournal(resource, storage));
    assert.equal(reloaded.restore().phase, 'pending');
    const calls = [];
    const handlers = Object.fromEntries(
      ['save', 'status', 'delete'].map((name) => [
        name,
        async (request, key) => {
          calls.push({ name, request, key });
          return confirmation(operation);
        }
      ])
    );
    const result = await reloaded.resume(handlers);
    assert.deepEqual(calls, [{ name: operation, ...initial }]);
    assert.equal(result.outcome.status, 'confirmed');
    const settled = JSON.parse(storage.text).record;
    assert.equal(settled.phase, 'settled');
    assert.equal(settled.key, initial.key);
    assert.ok(!storage.text.includes('cotton'));
    assert.ok(!Object.hasOwn(settled, 'attempt'));
    assert.ok(!Object.hasOwn(settled, 'request'));
    const oldUrl = new ProductWriteAttempt(memoryJournal(resource, storage));
    assert.equal(await oldUrl.resume(handlers), null);
    assert.equal(
      await oldUrl.execute(operation, requestFor(operation), async () => {
        throw new Error('must not send');
      }),
      null
    );
    assert.equal(calls.length, 1);
  });
}

test('duplicate tabs serialize owner IO and an acknowledged URL cannot become another create', async () => {
  const storage = state();
  const tabA = new ProductWriteAttempt(memoryJournal('new', storage));
  const tabB = new ProductWriteAttempt(memoryJournal('new', storage));
  let finish,
    sends = 0;
  const first = tabA.execute('save', save(), async () => {
    sends++;
    return new Promise((resolve) => {
      finish = resolve;
    });
  });
  const result = await tabB.execute('save', save(), async () => {
    sends++;
    return confirmation('save');
  });
  assert.equal(result.outcome.status, 'unknown');
  assert.ok(tabB.recoveryError);
  assert.equal(sends, 1);
  finish(confirmation('save'));
  await first;
  const again = await tabB.execute('save', save(), async () => {
    sends++;
    return confirmation('save');
  });
  assert.equal(again, null);
  assert.equal(sends, 1);
  assert.equal(tabB.recovery.phase, 'settled');
});

test('rejected tombstone requires an explicit distinct intent before a corrected operation', async () => {
  const storage = state();
  const writer = new ProductWriteAttempt(memoryJournal('new', storage));
  await writer.execute('save', save(), async () => ({
    status: 'rejected',
    code: 'PRODUCT_VALIDATION',
    message: 'Invalid'
  }));
  let sends = 0;
  const oldUrl = new ProductWriteAttempt(memoryJournal('new', storage));
  await oldUrl.execute('save', save(), async () => {
    sends++;
    return confirmation('save');
  });
  assert.equal(sends, 0);
  assert.equal(oldUrl.recovery.outcome.status, 'rejected');
  const next = new ProductWriteAttempt(
    memoryJournal(
      'new',
      state(),
      testScope,
      'f0348e73-1b85-41d4-972b-84bddbdf94d0'
    )
  );
  await next.execute('save', save(), async () => {
    sends++;
    return confirmation('save');
  });
  assert.equal(sends, 1);
});

for (const failure of ['read', 'write', 'ignored-write', 'lock']) {
  test(`unavailable ${failure} fails closed before owner IO`, async () => {
    const journal = new ProductWriteJournal(testScope, 'new', testIntent, {
      read: () => {
        if (failure === 'read') throw new Error('blocked');
        return null;
      },
      write: () => {
        if (failure === 'write') throw new Error('quota');
      },
      exclusive: async (work) => {
        if (failure === 'lock') throw new Error('busy');
        return work();
      }
    });
    let sends = 0;
    const writer = new ProductWriteAttempt(journal);
    const result = await writer.execute('save', save(), async () => {
      sends++;
      return confirmation('save');
    });
    assert.equal(sends, 0);
    assert.equal(result.outcome.status, 'unknown');
    assert.ok(writer.recoveryError);
  });
}

test('lost terminal storage acknowledgement retains the pending key and can replay it', async () => {
  let text = null,
    failTerminal = true;
  const journal = () =>
    new ProductWriteJournal(testScope, 'new', testIntent, {
      read: () => text,
      write: (value) => {
        if (failTerminal && JSON.parse(value).record.phase === 'settled')
          throw new Error('quota');
        text = value;
      },
      exclusive: async (work) => work()
    });
  const first = new ProductWriteAttempt(journal());
  const calls = [];
  const send = async (request, key) => {
    calls.push({ request, key });
    return confirmation('save');
  };
  assert.equal(
    (await first.execute('save', save(), send)).outcome.status,
    'unknown'
  );
  assert.equal(JSON.parse(text).record.phase, 'pending');
  failTerminal = false;
  const reloaded = new ProductWriteAttempt(journal());
  assert.equal(
    (await reloaded.resume({ save: send })).outcome.status,
    'confirmed'
  );
  assert.equal(calls.length, 2);
  assert.deepEqual(calls[0], calls[1]);
  assert.equal(JSON.parse(text).record.phase, 'settled');
});

for (const mutation of [
  'missing',
  'different-key',
  'changed-request',
  'corrupt'
]) {
  test(`a live editor cannot forget its admitted identity after ${mutation}`, async () => {
    const storage = state();
    const writer = new ProductWriteAttempt(memoryJournal('new', storage));
    await writer.execute('save', save(), async () => unknown);
    const key = writer.recovery.attempt.key;
    if (mutation === 'missing') storage.text = null;
    if (mutation === 'corrupt') storage.text = '{invalid';
    if (mutation === 'different-key') {
      const parsed = JSON.parse(storage.text);
      parsed.record.attempt.key = 'f0348e73-1b85-41d4-972b-84bddbdf94d0';
      storage.text = JSON.stringify(parsed);
    }
    if (mutation === 'changed-request') {
      const parsed = JSON.parse(storage.text);
      parsed.record.attempt.request.translations[0].title = 'Changed';
      storage.text = JSON.stringify(parsed);
    }
    let sends = 0;
    const result = await writer.execute('save', save(), async () => {
      sends++;
      return confirmation('save');
    });
    assert.equal(sends, 0);
    assert.equal(result.outcome.status, 'unknown');
    assert.equal(writer.recovery.attempt.key, key);
    assert.ok(writer.recoveryError);
  });
}

test('foreign scope/resource/intent and extra auth fields are not admitted as recovery', async () => {
  const storage = state();
  await new ProductWriteAttempt(memoryJournal('new', storage)).execute(
    'save',
    save(),
    async () => unknown
  );
  const original = storage.text;
  for (const mutate of [
    (raw) => {
      raw.scope.actorId = 'other';
    },
    (raw) => {
      raw.resource = 'other';
    },
    (raw) => {
      raw.intent = 'f0348e73-1b85-41d4-972b-84bddbdf94d0';
    },
    (raw) => {
      raw.record.attempt.request.scope.tenantId = 'other';
    },
    (raw) => {
      raw.record.attempt.request.token = 'secret';
    },
    (raw) => {
      raw.record.attempt.request.attributePatches[0].text = 123;
    },
    (raw) => {
      raw.record.attempt.key = 'not-a-key';
    }
  ]) {
    const raw = JSON.parse(original);
    mutate(raw);
    storage.text = JSON.stringify(raw);
    assert.throws(
      () => memoryJournal('new', storage).read(),
      /recovery is unavailable/
    );
  }
  storage.text = ' '.repeat(2 * 1024 * 1024 + 1);
  assert.throws(
    () => memoryJournal('new', storage).read(),
    /recovery is unavailable/
  );
});

function browser(
  t,
  href = 'https://admin.example/dashboard/product/new?locale=ru#draft'
) {
  const previousWindow = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const previousNavigator = Object.getOwnPropertyDescriptor(
    globalThis,
    'navigator'
  );
  const records = new Map();
  const historyState = { __NA: true, tree: ['dashboard'] };
  const win = {
    isSecureContext: true,
    location: { href },
    history: {
      state: historyState,
      replaceState: (value, _unused, url) => {
        assert.equal(value, historyState);
        win.location.href = url.href;
      }
    },
    localStorage: {
      getItem: (key) => records.get(key) ?? null,
      setItem: (key, value) => {
        records.set(key, value);
      }
    }
  };
  const nav = { locks: { request: async (_key, _options, work) => work({}) } };
  Object.defineProperty(globalThis, 'window', {
    configurable: true,
    value: win
  });
  Object.defineProperty(globalThis, 'navigator', {
    configurable: true,
    value: nav
  });
  t.after(() => {
    if (previousWindow)
      Object.defineProperty(globalThis, 'window', previousWindow);
    else delete globalThis.window;
    if (previousNavigator)
      Object.defineProperty(globalThis, 'navigator', previousNavigator);
    else delete globalThis.navigator;
  });
  return { win, nav, records };
}

test('browser intent preserves URL state, binds namespaces and only rotates explicitly', async (t) => {
  const { win, records } = browser(t);
  const first = browserProductWriteJournal(testScope, 'new');
  assert.ok(isProductEditorIntent(first.intent));
  const url = new URL(win.location.href);
  assert.equal(url.searchParams.get('locale'), 'ru');
  assert.equal(url.hash, '#draft');
  assert.equal(
    url.searchParams.get(PRODUCT_EDITOR_INTENT_QUERY_KEY),
    first.intent
  );
  assert.equal(
    browserProductWriteJournal(testScope, 'new').storageKey,
    first.storageKey
  );
  assert.notEqual(
    browserProductWriteJournal({ ...testScope, actorId: 'other' }, 'new')
      .storageKey,
    first.storageKey
  );
  assert.notEqual(
    browserProductWriteJournal(testScope, 'product').storageKey,
    first.storageKey
  );
  await new ProductWriteAttempt(first).execute(
    'save',
    save(),
    async () => unknown
  );
  const next = browserProductWriteJournal(testScope, 'new', true);
  assert.notEqual(next.storageKey, first.storageKey);
  assert.equal(next.read(), null);
  assert.equal(
    records.size,
    1,
    'rotation does not delete the unresolved old intent'
  );
});

for (const failure of [
  'insecure',
  'no-locks',
  'busy',
  'storage',
  'invalid-intent',
  'ambiguous-intent'
]) {
  test(`browser ${failure} cannot admit a fresh owner command`, async (t) => {
    const { win, nav } = browser(t);
    if (failure === 'insecure') win.isSecureContext = false;
    if (failure === 'no-locks') delete nav.locks;
    if (failure === 'busy')
      nav.locks.request = async (_key, _opts, work) => work(null);
    if (failure === 'storage')
      win.localStorage.setItem = () => {
        throw new Error('quota');
      };
    if (failure === 'invalid-intent')
      win.location.href =
        'https://admin.example/dashboard/product/new?editor_intent=bad';
    if (failure === 'ambiguous-intent')
      win.location.href = `https://admin.example/dashboard/product/new?editor_intent=${testIntent}&editor_intent=${testIntent}`;
    if (
      ['insecure', 'no-locks', 'invalid-intent', 'ambiguous-intent'].includes(
        failure
      )
    ) {
      assert.throws(
        () => browserProductWriteJournal(testScope, 'new'),
        /recovery is unavailable/
      );
      return;
    }
    let sends = 0;
    const writer = new ProductWriteAttempt(
      browserProductWriteJournal(testScope, 'new')
    );
    assert.equal(
      (
        await writer.execute('save', save(), async () => {
          sends++;
          return confirmation('save');
        })
      ).outcome.status,
      'unknown'
    );
    assert.equal(sends, 0);
  });
}

test('a native URL change cannot submit the old mounted intent and restoring its URL recovers the same key', async (t) => {
  const { win } = browser(t);
  const journal = browserProductWriteJournal(testScope, 'new');
  const writer = new ProductWriteAttempt(journal);
  let retainedKey;
  await writer.execute('save', save(), async (_request, key) => {
    retainedKey = key;
    return unknown;
  });
  const original = win.location.href;
  win.location.href = 'https://admin.example/dashboard/product/new';
  let sends = 0;
  assert.equal(
    (
      await writer.execute('save', save(), async () => {
        sends++;
        return confirmation('save');
      })
    ).outcome.status,
    'unknown'
  );
  assert.equal(sends, 0);
  win.location.href = original;
  let replayedKey;
  const result = await writer.resume({
    save: async (_request, key) => {
      replayedKey = key;
      return confirmation('save');
    }
  });
  assert.equal(result.outcome.status, 'confirmed');
  assert.equal(replayedKey, retainedKey);
});

test('JSON must not silently turn invalid integer/boolean/text projections into value clears', async () => {
  for (const patch of [
    { integer: NaN },
    { integer: Infinity },
    { integer: 1.5 },
    { boolean: 1 },
    { text: new Date() }
  ]) {
    const storage = state();
    const writer = new ProductWriteAttempt(memoryJournal('new', storage));
    const request = save();
    Object.assign(request.attributePatches[0], patch);
    let sends = 0;
    const result = await writer.execute('save', request, async () => {
      sends++;
      return confirmation('save');
    });
    assert.equal(sends, 0);
    assert.equal(storage.text, null);
    assert.equal(result.outcome.status, 'unknown');
  }
});

test('URL intent is checked inside the asynchronously acquired browser lock', async (t) => {
  const { win, nav } = browser(t);
  const writer = new ProductWriteAttempt(
    browserProductWriteJournal(testScope, 'new')
  );
  nav.locks.request = async (_key, _options, work) => {
    win.location.href = 'https://admin.example/dashboard/product/new';
    return work({});
  };
  let sends = 0;
  const result = await writer.execute('save', save(), async () => {
    sends++;
    return confirmation('save');
  });
  assert.equal(sends, 0);
  assert.equal(result.outcome.status, 'unknown');
});
