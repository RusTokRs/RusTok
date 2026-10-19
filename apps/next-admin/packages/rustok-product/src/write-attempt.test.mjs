/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import { memoryJournal, testScope } from './write-test-support.mjs';
import test from 'node:test';
import assert from 'node:assert/strict';
import { ProductWriteAttempt, UNKNOWN_PRODUCT_WRITE } from './write-attempt.ts';

const saved = { status: 'confirmed', value: { id: 'product', revision: 8 } };
const unknown = { status: 'unknown', message: UNKNOWN_PRODUCT_WRITE };
const refused = {
  status: 'rejected',
  code: 'PRODUCT_VALIDATION',
  message: 'Invalid request'
};
const payload = () => ({
  scope: { tenantId: 'tenant', actorId: 'actor' },
  isNew: true,
  translations: [{ locale: 'en', title: 'Chair' }],
  revision: 7,
  tags: [],
  attributePatches: [],
  activeLocale: 'en'
});

test('unknown/transport exceptions retain exactly the original immutable create and key', async () => {
  const attempt = new ProductWriteAttempt(memoryJournal());
  const request = payload();
  const calls = [];
  const send = async (snapshot, key) => {
    calls.push({ snapshot, key });
    assert.ok(Object.isFrozen(snapshot));
    assert.ok(Object.isFrozen(snapshot.translations[0]));
    if (calls.length === 1) throw new Error('production action exception');
    return saved;
  };
  const first = await attempt.execute('save', request, send);
  assert.equal(first.outcome.status, 'unknown');
  request.translations[0].title = 'Changed after request';
  request.revision = 99;
  const second = await attempt.execute('save', request, send);
  assert.equal(second.outcome.status, 'confirmed');
  assert.equal(calls[0].key, calls[1].key);
  assert.match(calls[0].key, /^[a-f0-9-]{36}$/);
  assert.deepEqual(calls[0].snapshot, calls[1].snapshot);
  assert.equal(calls[1].snapshot.translations[0].title, 'Chair');
  assert.equal(calls[1].snapshot.revision, 7);
  assert.equal(attempt.pendingOperation, null);
});

test('synchronous lock blocks duplicate submits before React re-renders', async () => {
  const attempt = new ProductWriteAttempt(memoryJournal());
  let finish;
  let calls = 0;
  const first = attempt.execute('save', payload(), () => {
    calls++;
    return new Promise((resolve) => {
      finish = resolve;
    });
  });
  assert.equal(attempt.isBusy, true);
  assert.equal(
    await attempt.execute('save', payload(), () => {
      calls++;
      return saved;
    }),
    null
  );
  assert.equal(calls, 1);
  finish(saved);
  await first;
  assert.equal(attempt.isBusy, false);
});

test('an unresolved save cannot be replaced with status or deletion', async () => {
  const attempt = new ProductWriteAttempt(memoryJournal());
  await attempt.execute('save', payload(), async () => unknown);
  for (const operation of ['status', 'delete']) {
    const result = await attempt.execute(operation, { id: 'other' }, async () =>
      assert.fail('must not issue another command')
    );
    assert.equal(result.outcome.status, 'unknown');
    assert.equal(attempt.pendingOperation, 'save');
  }
});

test('definitive refusal settles the intent and an explicitly new intent receives a new key', async () => {
  const attempt = new ProductWriteAttempt(memoryJournal());
  const keys = [];
  const result = await attempt.execute('save', payload(), async (_, key) => {
    keys.push(key);
    return refused;
  });
  assert.equal(result.outcome.status, 'rejected');
  assert.equal(attempt.pendingOperation, null);
  assert.equal(
    await attempt.execute('save', payload(), async () => saved),
    null
  );
  await new ProductWriteAttempt(
    memoryJournal(
      'new',
      { text: null, locked: false },
      testScope,
      'f0348e73-1b85-41d4-972b-84bddbdf94d0'
    )
  ).execute('save', payload(), async (_, key) => {
    keys.push(key);
    return saved;
  });
  assert.notEqual(keys[0], keys[1]);
});

test('retrying status uses its original target and predecessor, not current controls', async () => {
  const attempt = new ProductWriteAttempt(memoryJournal('product'));
  let key;
  await attempt.execute(
    'status',
    { scope: testScope, id: 'product', status: 'ACTIVE', revision: 7 },
    async (_, value) => {
      key = value;
      return unknown;
    }
  );
  const result = await attempt.execute(
    'status',
    { scope: testScope, id: 'product', status: 'DRAFT', revision: 88 },
    async (request, value) => {
      assert.equal(value, key);
      assert.deepEqual(request, {
        scope: testScope,
        id: 'product',
        status: 'ACTIVE',
        revision: 7
      });
      return saved;
    }
  );
  assert.equal(result.outcome.status, 'confirmed');
  assert.equal(result.request.status, 'ACTIVE');
});

test('delete retries keep their key and accept only an explicit acknowledgement', async () => {
  const attempt = new ProductWriteAttempt(memoryJournal('product'));
  let key;
  await attempt.execute(
    'delete',
    { scope: testScope, id: 'product' },
    async (_, value) => {
      key = value;
      throw new Error('lost delete response');
    }
  );
  const result = await attempt.execute(
    'delete',
    { id: 'changed' },
    async (request, value) => {
      assert.equal(request.id, 'product');
      assert.equal(value, key);
      return { status: 'confirmed', value: { id: 'product', revision: null } };
    }
  );
  assert.equal(result.outcome.status, 'confirmed');
});

for (const malformed of [
  undefined,
  {},
  { status: 'confirmed' },
  { status: 'confirmed', value: { id: 'product' } },
  { status: 'confirmed', value: { id: 'product', revision: null } },
  { status: 'rejected' }
]) {
  test(`malformed acknowledgement is unknown: ${JSON.stringify(malformed)}`, async () => {
    const attempt = new ProductWriteAttempt(memoryJournal());
    const result = await attempt.execute(
      'save',
      payload(),
      async () => malformed
    );
    assert.equal(result.outcome.status, 'unknown');
    assert.equal(attempt.pendingOperation, 'save');
  });
}

test('an acknowledgement for a different existing Product is not a success', async () => {
  const attempt = new ProductWriteAttempt(memoryJournal('product'));
  const result = await attempt.execute(
    'status',
    { scope: testScope, id: 'product', status: 'ACTIVE', revision: 7 },
    async () => ({
      status: 'confirmed',
      value: { id: 'other-product', revision: 8 }
    })
  );
  assert.equal(result.outcome.status, 'unknown');
  assert.equal(attempt.pendingOperation, 'status');
});

test('unknown retries retain their original actor/tenant scope', async () => {
  const attempt = new ProductWriteAttempt(memoryJournal());
  await attempt.execute('save', payload(), async () => unknown);
  const changed = payload();
  changed.scope = { tenantId: 'other-tenant', actorId: 'other-actor' };
  await attempt.execute('save', changed, async (request) => {
    assert.deepEqual(request.scope, { tenantId: 'tenant', actorId: 'actor' });
    return saved;
  });
});
