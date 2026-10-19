/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import { memoryJournal } from './write-test-support.mjs';
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import vm from 'node:vm';
import { parsePriceAmount } from './money.ts';
import { PRODUCT_RECOVERY_UNAVAILABLE } from './write-journal.ts';
import { ProductWriteAttempt, UNKNOWN_PRODUCT_WRITE } from './write-attempt.ts';

const editor = readFileSync(
  new URL('./pages/product-editor-page.tsx', import.meta.url),
  'utf8'
);
const actions = readFileSync(
  new URL('../../../src/app/dashboard/product/actions.ts', import.meta.url),
  'utf8'
);
const transport = readFileSync(
  new URL('../../../src/shared/api/graphql.ts', import.meta.url),
  'utf8'
);
const outcomes = readFileSync(
  new URL('./api/write-outcome.ts', import.meta.url),
  'utf8'
);
const GraphqlError = vm.runInNewContext(
  stripTypeScriptTypes(
    transport.slice(
      transport.indexOf('export class GraphqlError'),
      transport.indexOf('/** Read the host-resolved')
    )
  ).replace('export class', 'class') + '\nGraphqlError;',
  { Error }
);
const productWriteOutcome = vm.runInNewContext(
  stripTypeScriptTypes(
    outcomes.slice(outcomes.indexOf('const definitiveRefusals:'))
  ).replace('export async function', 'async function') +
    '\nproductWriteOutcome;',
  { GraphqlError, UNKNOWN_PRODUCT_WRITE }
);
const confirmed = {
  status: 'confirmed',
  value: { id: 'created', revision: 1 }
};
const scope = { tenantId: 'tenant', actorId: 'actor' };
const payload = () => ({
  scope,
  isNew: true,
  translations: [{ locale: 'en', title: 'Chair' }],
  tags: [],
  attributePatches: [{ attributeId: 'material', kind: 'TEXT', text: 'cotton' }],
  activeLocale: 'en',
  status: 'ACTIVE'
});

function saveHandler(send) {
  const state = { errors: [], pending: null, dirtyCleared: 0, paths: [] };
  const context = {
    recoveryReady: true,
    PRODUCT_RECOVERY_UNAVAILABLE,
    setRecoveryReady: () => {},
    setRecoveryError: () => {},
    isLoadingForm: false,
    isSaving: false,
    isNew: true,
    initialProduct: null,
    writeAttempt: { current: new ProductWriteAttempt(memoryJournal()) },
    createdProduct: { current: null },
    deletedProduct: { current: false },
    writeScope: scope,
    translations: { en: { locale: 'en', title: 'Chair' } },
    activeLocale: 'en',
    vendor: '',
    sellerId: '',
    productType: 'simple',
    shippingProfileSlug: '',
    selectedCategoryId: 'category',
    tags: [],
    status: 'ACTIVE',
    documentRevision: 7,
    dirtyAttributeIds: new Set(['material']),
    attributeValues: {
      material: { attributeId: 'material', kind: 'TEXT', text: 'cotton' }
    },
    newVariantDraft: {
      sku: 'sku',
      barcode: '',
      priceAmount: '1.25',
      currencyCode: 'USD',
      inventoryQuantity: 0,
      inventoryPolicy: 'deny'
    },
    setErrorMessage: (message) => {
      if (message) state.errors.push(message);
    },
    setSuccessMessage: () => {},
    setIsSaving: () => {},
    setDocumentRevision: (value) => {
      state.revision = value;
    },
    setPendingOperation: (value) => {
      state.pending = value;
    },
    setDirtyAttributeIds: () => {
      state.dirtyCleared++;
    },
    parsePriceAmount,
    onSaveProduct: send,
    onChangeStatus: undefined,
    onDeleteProduct: undefined,
    router: { push: (path) => state.paths.push(path) },
    Set,
    Error
  };
  context.syncRecoveryState = () => {
    state.pending = context.writeAttempt.current.pendingOperation;
    state.settled = context.writeAttempt.current.recovery;
  };
  const resume = editor.slice(
    editor.indexOf('  const handleResume ='),
    editor.indexOf('  const handleCorrectDraft =')
  );
  const source =
    resume +
    editor.slice(
      editor.indexOf('  const handleSave ='),
      editor.indexOf('  const handleStatusChange =')
    );
  return {
    run: vm.runInNewContext(
      stripTypeScriptTypes(source) + '\nhandleSave;',
      context
    ),
    state,
    context
  };
}

test('actual save handler retries the original complete request after an unknown create', async () => {
  const calls = [];
  const { run, state, context } = saveHandler(async (request, key) => {
    calls.push({ request, key });
    if (calls.length === 1) throw new Error('redacted production error');
    return confirmed;
  });
  await run();
  assert.equal(state.pending, 'save');
  assert.equal(state.dirtyCleared, 0);
  assert.equal(state.errors[0], UNKNOWN_PRODUCT_WRITE);
  context.translations.en.title = 'Different new title';
  context.attributeValues.material.text = 'wool';
  context.documentRevision = 100;
  context.writeScope = { tenantId: 'other', actorId: 'other' };
  await run();
  assert.equal(calls[0].key, calls[1].key);
  assert.deepEqual(calls[0].request, calls[1].request);
  assert.equal(calls[1].request.translations[0].title, 'Chair');
  assert.equal(calls[1].request.attributePatches[0].text, 'cotton');
  assert.equal(calls[1].request.revision, 7);
  assert.deepEqual(calls[1].request.scope, scope);
  assert.equal(state.dirtyCleared, 0);
  assert.equal(state.pending, null);
  assert.equal(state.revision, undefined);
  assert.equal(state.settled.phase, 'settled');
  // The component may remain mounted briefly while navigation is scheduled.
  await run();
  assert.equal(
    calls.length,
    2,
    'acknowledged create cannot accidentally create again'
  );
  assert.deepEqual(
    state.paths,
    [],
    'recovered acknowledgement requires opening a fresh owner document'
  );
});

test('actual save handler admits only one create during a double click', async () => {
  let resolve;
  let count = 0;
  const { run } = saveHandler(async () => {
    count++;
    return new Promise((done) => {
      resolve = done;
    });
  });
  const first = run();
  await run();
  assert.equal(count, 1);
  resolve(confirmed);
  await first;
});

test('only bounded nonretryable request refusals release an attempt', async () => {
  for (const code of [
    'PRODUCT_VALIDATION',
    'PRODUCT_REVISION_CONFLICT',
    'DUPLICATE_HANDLE'
  ]) {
    const result = await productWriteOutcome(async () => {
      throw new GraphqlError(
        'private backend details',
        code,
        false,
        'rejected'
      );
    });
    assert.equal(result.status, 'rejected');
    assert.equal(result.code, code);
    assert.ok(!result.message.includes('private'));
  }
  for (const error of [
    new Error('redacted'),
    new GraphqlError('private', 'PRODUCT_VALIDATION'),
    new GraphqlError('private', 'PRODUCT_VALIDATION', false),
    new GraphqlError('private', 'PRODUCT_VALIDATION', false, 'unknown'),
    new GraphqlError('private', 'PRODUCT_VALIDATION', true),
    new GraphqlError('private', 'UNAUTHORIZED', false),
    new GraphqlError('private', 'PRODUCT_ACCESS_DENIED', false),
    new GraphqlError('private', 'PRODUCT_OPERATION_FAILED', false),
    new GraphqlError('private', 'PRODUCT_NOT_FOUND', false)
  ]) {
    const result = await productWriteOutcome(async () => {
      throw error;
    });
    assert.equal(result.status, 'unknown');
    assert.equal(result.message, UNKNOWN_PRODUCT_WRITE);
  }
});

function serverAction(name, overrides = {}) {
  const start = actions.indexOf(`export async function ${name}(`);
  const end = actions.indexOf('\nexport async function ', start + 1);
  const method = actions.slice(start, end < 0 ? undefined : end);
  const refreshStart = actions.indexOf(
    'async function refreshProductAfterWrite('
  );
  const refreshEnd = actions.indexOf('\nexport async function ', refreshStart);
  const refresh = actions.slice(refreshStart, refreshEnd);
  const calls = [];
  const context = {
    productWriteOutcome,
    Error,
    getProductWriteAuthOpts: async (expected) => {
      assert.deepEqual(expected, scope);
      return { token: 'auth' };
    },
    createProduct: async (opts, request, key) => {
      calls.push({ opts, request, key });
      return { id: 'created', revision: 1 };
    },
    updateProductDetail: async () => assert.fail('create must not update'),
    deleteProductDetail: async () => assert.fail('create must not compensate'),
    revalidatePath: () => {},
    ...overrides
  };
  return {
    run: vm.runInNewContext(
      stripTypeScriptTypes(
        method.replace('export async function', 'async function') +
          (method.includes('async function refreshProductAfterWrite(')
            ? ''
            : refresh)
      ) + `\n${name};`,
      context
    ),
    calls
  };
}

test('Server Action forwards a retained key with EAV, without compensating a create', async () => {
  const { run, calls } = serverAction('saveProductAction');
  const result = await run(payload(), 'browser-attempt');
  assert.equal(result.status, 'confirmed');
  assert.equal(calls.length, 1);
  assert.equal(calls[0].key, 'browser-attempt');
  assert.equal(calls[0].request.publish, true);
  assert.deepEqual(
    JSON.parse(JSON.stringify(calls[0].request.attributeValues)),
    [{ locale: 'en', patches: payload().attributePatches }]
  );
});

test('cache invalidation failure cannot turn acknowledged commit into another create', async () => {
  const { run } = serverAction('saveProductAction', {
    revalidatePath: () => {
      throw new Error('cache failed');
    }
  });
  assert.equal((await run(payload(), 'attempt')).status, 'confirmed');
});

test('Server Action owner/network exceptions are serialized, not thrown through production redaction', async () => {
  const { run } = serverAction('saveProductAction', {
    createProduct: async () => {
      throw new Error('secret stack trace');
    }
  });
  const result = await run(payload(), 'attempt');
  assert.equal(result.status, 'unknown');
  assert.equal(result.message, UNKNOWN_PRODUCT_WRITE);
});

test('captured scope is compared with the single authenticated session before owner IO', async () => {
  let session = {
    user: {
      id: 'actor',
      tenantId: 'tenant',
      tenantSlug: 'tenant',
      rustokToken: 'token'
    }
  };
  let reads = 0;
  const source = actions.slice(
    actions.indexOf('async function getAuthOpts('),
    actions.indexOf('export async function saveProductAction(')
  );
  const getOpts = vm.runInNewContext(
    stripTypeScriptTypes(source) + '\ngetProductWriteAuthOpts;',
    {
      auth: async () => {
        reads++;
        return session;
      },
      graphqlRequest: () => assert.fail('auth cannot write'),
      Error
    }
  );
  const key = 'f0348e73-1b85-41d4-972b-84bddbdf94d0';
  assert.equal((await getOpts(scope, key)).token, 'token');
  assert.equal(reads, 1);
  session = { user: { ...session.user, tenantId: 'other-tenant' } };
  await assert.rejects(getOpts(scope, key), /context changed/);
  session = {
    user: { ...session.user, tenantId: 'tenant', id: 'other-actor' }
  };
  await assert.rejects(getOpts(scope, key), /context changed/);
  await assert.rejects(getOpts(undefined, key), /context is missing/);
  await assert.rejects(
    getOpts(scope, undefined),
    /identity is missing or invalid/
  );
  await assert.rejects(
    getOpts(scope, 'not-an-attempt'),
    /identity is missing or invalid/
  );
});

test('invalid initial money or GraphQL Int is rejected locally before a pending attempt exists', async () => {
  for (const patch of [
    { priceAmount: 'bad' },
    { inventoryQuantity: 0.5 },
    { inventoryQuantity: 2147483648 }
  ]) {
    const { run, state, context } = saveHandler(() =>
      assert.fail('invalid scalar must not reach the action')
    );
    Object.assign(context.newVariantDraft, patch);
    await run();
    assert.equal(context.writeAttempt.current.pendingOperation, null);
    assert.equal(state.dirtyCleared, 0);
    assert.equal(state.errors.length, 1);
  }
});

test('portal/child mutation guard observes the current pending attempt, not stale React state', async () => {
  const attempt = new ProductWriteAttempt(memoryJournal());
  const start = editor.indexOf('  const assertEditorialWriteResolved =');
  const end = editor.indexOf('\n  return (', start);
  assert.ok(start >= 0 && end > start);
  const guard = vm.runInNewContext(
    stripTypeScriptTypes(editor.slice(start, end)) +
      '\nassertEditorialWriteResolved;',
    {
      recoveryReady: true,
      writeAttempt: { current: attempt },
      deletedProduct: { current: false },
      Error
    }
  );
  guard();
  await attempt.execute('save', payload(), async () => ({
    status: 'unknown',
    message: UNKNOWN_PRODUCT_WRITE
  }));
  assert.throws(guard, /Resolve the previous Product operation/);
});

test('server prop refresh cannot silently replace the dirty document predecessor', () => {
  assert.ok(!editor.includes('setDocumentRevision(initialProduct?.revision'));
  assert.match(editor, /initialProduct\?\.revision \?\? null/);
});

for (const [name, field, mutation] of [
  ['updateProductDetail', 'updateProduct', 'UPDATE_PRODUCT_MUTATION'],
  ['deleteProductDetail', 'deleteProduct', 'DELETE_PRODUCT_MUTATION']
]) {
  test(`canonical ${name} adapter forwards the supplied key instead of minting another`, async () => {
    const source = readFileSync(
      new URL('./api/products.ts', import.meta.url),
      'utf8'
    );
    const start = source.indexOf(`export async function ${name}(`);
    const end = source.indexOf('\nexport async function ', start + 1);
    const method = source
      .slice(start, end)
      .replace('export async function', 'async function');
    const adapter = vm.runInNewContext(
      stripTypeScriptTypes(method) + `\n${name};`,
      {
        [mutation]: 'canonical-mutation-document',
        Error,
        crypto: {
          randomUUID: () => assert.fail('a supplied key must not be replaced')
        }
      }
    );
    const keys = [];
    const opts = {
      token: 'token',
      tenantSlug: 'tenant',
      tenantId: 'tenant',
      graphql: async (_, variables) => {
        keys.push(variables.idempotencyKey);
        return {
          [field]:
            field === 'deleteProduct' ? true : { id: 'product', revision: 8 }
        };
      }
    };
    for (let i = 0; i < 2; i++) {
      if (field === 'deleteProduct')
        await adapter(opts, 'product', 'retained-key');
      else
        await adapter(
          opts,
          'product',
          { revision: 7, status: 'ACTIVE' },
          'retained-key'
        );
    }
    assert.deepEqual(keys, ['retained-key', 'retained-key']);
  });
}

test('recovered save bypasses current empty fields, category loading and invalid draft money', async () => {
  const storage = { text: null, locked: false };
  const writer = new ProductWriteAttempt(memoryJournal('new', storage));
  let retained;
  await writer.execute('save', payload(), async (request, key) => {
    retained = { request, key };
    return { status: 'unknown', message: UNKNOWN_PRODUCT_WRITE };
  });
  const calls = [];
  const { run, context, state } = saveHandler(async (request, key) => {
    calls.push({ request, key });
    return confirmed;
  });
  context.writeAttempt.current = new ProductWriteAttempt(
    memoryJournal('new', storage)
  );
  context.writeAttempt.current.restore();
  context.translations = {};
  context.isLoadingForm = true;
  context.newVariantDraft.priceAmount = 'invalid';
  await run();
  assert.deepEqual(calls, [retained]);
  assert.equal(state.settled.phase, 'settled');
  assert.equal(
    state.dirtyCleared,
    0,
    'recovery does not overwrite a separate current draft'
  );
});

for (const operation of ['status', 'delete']) {
  test(`recovery dispatches original ${operation} even without an initial Product document`, async () => {
    const storage = { text: null, locked: false };
    const request = {
      id: 'product',
      scope,
      ...(operation === 'status' ? { status: 'ACTIVE', revision: 7 } : {})
    };
    const writer = new ProductWriteAttempt(memoryJournal('product', storage));
    let retainedKey;
    await writer.execute(operation, request, async (_request, key) => {
      retainedKey = key;
      return { status: 'unknown', message: UNKNOWN_PRODUCT_WRITE };
    });
    const { run, context, state } = saveHandler(async () =>
      assert.fail('must dispatch the retained operation, not save')
    );
    context.writeAttempt.current = new ProductWriteAttempt(
      memoryJournal('product', storage)
    );
    context.writeAttempt.current.restore();
    context.initialProduct = null;
    context.translations = {};
    const calls = [];
    context.onChangeStatus = async (...args) => {
      calls.push(args);
      return { status: 'confirmed', value: { id: 'product', revision: 8 } };
    };
    context.onDeleteProduct = async (...args) => {
      calls.push(args);
      return { status: 'confirmed', value: { id: 'product', revision: null } };
    };
    await run();
    assert.deepEqual(
      calls,
      operation === 'status'
        ? [['product', 'ACTIVE', 7, retainedKey, scope]]
        : [['product', retainedKey, scope]]
    );
    assert.equal(state.settled.phase, 'settled');
  });
}
