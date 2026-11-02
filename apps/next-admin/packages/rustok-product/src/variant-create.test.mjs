/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import vm from 'node:vm';
import { ProductWriteAttempt } from './write-attempt.ts';
import { memoryJournal, testScope } from './write-test-support.mjs';
import {
  isVariantCreateInput,
  prepareVariantCreateInput,
  isVariantCreateRefusalCode
} from './variant-create.ts';

const id = (value) =>
  `00000000-0000-4000-8000-${String(value).padStart(12, '0')}`;
const request = () => ({
  scope: testScope,
  id: id(1),
  locale: 'ru',
  input: prepareVariantCreateInput({
    sku: ' original SKU ',
    barcode: '123456',
    shippingProfileSlug: 'parcel',
    axisValues: [{ attributeId: id(2), optionId: id(3) }],
    prices: [
      { currencyCode: 'usd', amount: '12.000001', compareAtAmount: '15.00' },
      { currencyCode: 'JPY', amount: '0' }
    ],
    inventoryQuantity: 17,
    inventoryPolicy: 'continue'
  })
});
const confirmed = {
  status: 'confirmed',
  value: { id: id(1), revision: null, variantId: id(4) }
};
const unknown = { status: 'unknown', message: 'Response lost' };

test('preparation retains create text semantics, all fields, exact zero/decimal prices and explicit defaults', () => {
  const value = request().input;
  assert.equal(value.sku, ' original SKU ');
  assert.equal(value.barcode, '123456');
  assert.equal(value.shippingProfileSlug, 'parcel');
  assert.deepEqual(value.prices, [
    { currencyCode: 'USD', amount: '12.000001', compareAtAmount: '15' },
    { currencyCode: 'JPY', amount: '0', compareAtAmount: null }
  ]);
  assert.deepEqual(prepareVariantCreateInput({ prices: [] }), {
    sku: null,
    barcode: null,
    shippingProfileSlug: null,
    axisValues: [],
    prices: [],
    inventoryQuantity: 0,
    inventoryPolicy: 'deny'
  });
});

test('unsupported fields, invalid/duplicate UUID assignments, precision loss and bounds refuse before IO', () => {
  for (const change of [
    (input) => {
      input.weight = '2';
    },
    (input) => {
      input.prices[0].channelSlug = 'not-supported-by-this-form';
    },
    (input) => {
      input.axisValues[0].label = 'must not silently disappear';
    },
    (input) => {
      input.axisValues[0].optionId = 'bad';
    },
    (input) => {
      input.axisValues.push(input.axisValues[0]);
    },
    (input) => {
      input.inventoryQuantity = -1;
    },
    (input) => {
      input.inventoryQuantity = 2147483648;
    },
    (input) => {
      input.prices[0].amount = '0.00000000000000000000000000001';
    },
    (input) => {
      input.prices[0].amount = '79228162514264337593543950336';
    },
    (input) => {
      input.prices = Array.from({ length: 65 }, () => ({
        currencyCode: 'USD',
        amount: '1'
      }));
    }
  ]) {
    const input = structuredClone(request().input);
    change(input);
    assert.throws(() => prepareVariantCreateInput(input));
    assert.equal(isVariantCreateInput(input), false);
  }
});

test('unknown creation freezes the full request/key/locale and reload replays without Product metadata', async () => {
  const storage = { text: null, locked: false };
  const first = new ProductWriteAttempt(memoryJournal(id(1), storage));
  const original = request();
  let sent;
  await first.execute('variant', original, async (snapshot, key) => {
    const durable = JSON.parse(storage.text).record;
    assert.deepEqual(durable.attempt.request, snapshot);
    assert.equal(durable.attempt.key, key);
    assert.ok(Object.isFrozen(snapshot.input.prices[0]));
    sent = { snapshot, key };
    return unknown;
  });
  original.input.axisValues[0].optionId = id(9);
  original.input.prices[0].amount = '999';
  original.input.inventoryQuantity = 0;
  original.input.barcode = null;
  original.input.shippingProfileSlug = null;
  original.locale = 'en';
  const reloaded = new ProductWriteAttempt(memoryJournal(id(1), storage));
  const result = await reloaded.resume({
    variant: async (snapshot, key) => {
      assert.deepEqual({ snapshot, key }, sent);
      return confirmed;
    }
  });
  assert.equal(result.outcome.status, 'confirmed');
  assert.equal(reloaded.recovery.operation, 'variant');
  assert.deepEqual(reloaded.recovery.request, sent.snapshot);
  assert.equal(reloaded.recovery.outcome.value.variantId, id(4));
});

test('a pending SKU fences other operations and a foreign context cannot send its recovery', async () => {
  const storage = { text: null, locked: false };
  const writer = new ProductWriteAttempt(memoryJournal(id(1), storage));
  await writer.execute('variant', request(), async () => unknown);
  let calls = 0;
  const wrongOperation = await writer.execute(
    'delete',
    { id: id(1), scope: testScope },
    async () => {
      calls++;
      return confirmed;
    }
  );
  assert.equal(wrongOperation.outcome.status, 'unknown');
  const foreign = new ProductWriteAttempt(
    memoryJournal(id(1), storage, { ...testScope, actorId: 'other' })
  );
  assert.throws(() => foreign.restore());
  const result = await foreign.execute('variant', request(), async () => {
    calls++;
    return confirmed;
  });
  assert.equal(result.outcome.status, 'unknown');
  assert.equal(calls, 0);
});

test('malformed acknowledgements and unrecognized/admission refusals never settle the caller', async () => {
  for (const outcome of [
    { status: 'confirmed', value: { id: id(1), revision: null } },
    {
      status: 'confirmed',
      value: { id: id(1), revision: 2, variantId: id(4) }
    },
    {
      status: 'confirmed',
      value: { id: id(9), revision: null, variantId: id(4) }
    },
    {
      status: 'confirmed',
      value: { id: id(1), revision: null, variantId: 'bad' }
    },
    {
      status: 'rejected',
      code: 'UNAUTHORIZED',
      message: 'not an owner decision'
    },
    {
      status: 'rejected',
      code: 'PRODUCT_OPERATION_FAILED',
      message: 'receipt mismatch'
    }
  ]) {
    const writer = new ProductWriteAttempt(memoryJournal(id(1)));
    const result = await writer.execute(
      'variant',
      request(),
      async () => outcome
    );
    assert.equal(result.outcome.status, 'unknown');
    assert.equal(writer.pendingOperation, 'variant');
  }
  assert.equal(isVariantCreateRefusalCode('DUPLICATE_SKU'), true);
});

test('terminal rejection retains the full correction draft and cannot admit another request in the old intent', async () => {
  const storage = { text: null, locked: false };
  const original = request();
  const writer = new ProductWriteAttempt(memoryJournal(id(1), storage));
  await writer.execute('variant', original, async () => ({
    status: 'rejected',
    code: 'DUPLICATE_SKU',
    message: 'Already used'
  }));
  const reloaded = new ProductWriteAttempt(memoryJournal(id(1), storage));
  assert.deepEqual(reloaded.restore().request, original);
  assert.equal(
    await reloaded.execute('variant', request(), async () =>
      assert.fail('terminal intent is immutable')
    ),
    null
  );
});

for (const change of ['missing', 'corrupt', 'key', 'body']) {
  test(`a late acknowledgement cannot overwrite ${change} storage`, async () => {
    const storage = { text: null, locked: false };
    const writer = new ProductWriteAttempt(memoryJournal(id(1), storage));
    let altered;
    const result = await writer.execute('variant', request(), async () => {
      if (change === 'missing') storage.text = null;
      else if (change === 'corrupt') storage.text = '{bad';
      else {
        const raw = JSON.parse(storage.text);
        if (change === 'key') raw.record.attempt.key = id(9);
        else raw.record.attempt.request.input.inventoryQuantity = 99;
        storage.text = JSON.stringify(raw);
      }
      altered = storage.text;
      return confirmed;
    });
    assert.equal(result.outcome.status, 'unknown');
    assert.equal(writer.pendingOperation, 'variant');
    assert.equal(storage.text, altered);
    assert.ok(writer.recoveryError);
  });
}

test('failed terminal readback recovers its exact forward step, never a different terminal request', async () => {
  const storage = { text: null, locked: false };
  const writer = new ProductWriteAttempt(memoryJournal(id(1), storage));
  await writer.execute('variant', request(), async () => unknown);
  const pending = JSON.parse(storage.text).record;
  const frame = JSON.parse(storage.text);
  frame.record = {
    phase: 'settled',
    operation: 'variant',
    key: pending.attempt.key,
    request: pending.attempt.request,
    outcome: confirmed
  };
  storage.text = JSON.stringify(frame);
  assert.equal(writer.restore().phase, 'settled');
  // Same key but another full terminal request is not a forward successor.
  const otherStorage = { text: null, locked: false };
  const other = new ProductWriteAttempt(memoryJournal(id(1), otherStorage));
  await other.execute('variant', request(), async () => unknown);
  const otherFrame = JSON.parse(otherStorage.text);
  otherFrame.record = {
    ...frame.record,
    key: otherFrame.record.attempt.key,
    request: { ...request(), locale: 'en' }
  };
  otherStorage.text = JSON.stringify(otherFrame);
  assert.throws(() => other.restore());
});

test('the actual GraphQL SDK keeps the supplied caller key, complete body and frozen locale', async () => {
  const source = readFileSync(
    new URL('./api/products.ts', import.meta.url),
    'utf8'
  );
  const body = source.slice(
    source.indexOf('export async function createVariant('),
    source.indexOf('export async function updateVariant(')
  );
  const createVariant = vm.runInNewContext(
    stripTypeScriptTypes(body.replace('export async', 'async')) +
      '\ncreateVariant;',
    { CREATE_VARIANT_MUTATION: 'createProductVariant' }
  );
  let captured;
  const original = request();
  const key = id(8);
  const result = await createVariant(
    {
      token: 'test',
      tenantSlug: 'shop',
      tenantId: testScope.tenantId,
      locale: original.locale,
      graphql: async (...args) => {
        captured = args;
        return { createProductVariant: { id: id(4) } };
      }
    },
    original.id,
    original.input,
    key
  );
  assert.equal(result.id, id(4));
  assert.deepEqual(JSON.parse(JSON.stringify(captured[1])), {
    idempotencyKey: key,
    productId: original.id,
    input: original.input
  });
  assert.equal(captured[4].locale, 'ru');
});

test('the actual server action binds scope/key and does not rewrite the body or lose success on cache failure', async () => {
  const source = readFileSync(
    new URL('../../../src/app/dashboard/product/actions.ts', import.meta.url),
    'utf8'
  );
  const action = source.slice(
    source.indexOf('export async function addVariantAction('),
    source.indexOf('export async function updateVariantAction(')
  );
  const refresh = source.slice(
    source.indexOf('async function refreshProductAfterWrite('),
    source.indexOf('export async function addVariantAction(')
  );
  const calls = [];
  const original = request();
  const key = id(8);
  const run = vm.runInNewContext(
    stripTypeScriptTypes((refresh + action).replace('export async', 'async')) +
      '\naddVariantAction;',
    {
      getProductWriteAuthOpts: async (scope, supplied) => {
        assert.equal(supplied, key);
        if (scope.actorId !== testScope.actorId)
          throw new Error('Context changed');
        return { locale: 'en', token: 'test' };
      },
      isVariantCreateInput,
      productWriteOutcome: async (write) => {
        try {
          return { status: 'confirmed', value: await write() };
        } catch {
          return unknown;
        }
      },
      createVariant: async (...args) => {
        calls.push(args);
        return { id: id(4) };
      },
      revalidatePath: () => {
        throw new Error('cache unavailable after owner success');
      }
    }
  );
  const outcome = await run(original, key);
  assert.equal(outcome.status, 'confirmed');
  assert.equal(outcome.value.id, original.id);
  assert.equal(outcome.value.revision, null);
  assert.equal(outcome.value.variantId, id(4));
  assert.equal(calls[0][0].locale, original.locale);
  assert.equal(calls[0][2], original.input);
  assert.equal(calls[0][3], key);
  assert.equal(
    (await run({ ...original, scope: { ...testScope, actorId: 'other' } }, key))
      .status,
    'unknown'
  );
  assert.equal(
    (
      await run(
        { ...original, input: { ...original.input, inventoryQuantity: -1 } },
        key
      )
    ).status,
    'unknown'
  );
  assert.equal(calls.length, 1);
});
