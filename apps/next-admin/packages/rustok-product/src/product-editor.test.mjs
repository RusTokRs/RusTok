/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import { memoryJournal, testScope } from './write-test-support.mjs';
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import vm from 'node:vm';
import { ProductWriteAttempt, UNKNOWN_PRODUCT_WRITE } from './write-attempt.ts';

const editor = readFileSync(
  new URL('./pages/product-editor-page.tsx', import.meta.url),
  'utf8'
);
const actions = readFileSync(
  new URL('../../../src/app/dashboard/product/actions.ts', import.meta.url),
  'utf8'
);
const statusHandler = editor.slice(
  editor.indexOf('  const handleStatusChange ='),
  editor.indexOf('  const handleDelete =')
);
const statusAction = actions.slice(
  actions.indexOf('export async function changeProductStatusAction('),
  actions.indexOf('export async function deleteProductAction(')
);

function loadHandler(overrides = {}) {
  const writes = [];
  const state = {
    status: 'DRAFT',
    revision: 7,
    saving: false,
    error: null,
    success: null
  };
  const context = {
    Error,
    recoveryReady: true,
    isSaving: false,
    isNew: false,
    initialProduct: { id: 'product' },
    documentRevision: 7,
    writeScope: { tenantId: 'tenant', actorId: 'actor' },
    writeAttempt: {
      current: new ProductWriteAttempt(memoryJournal('product'))
    },
    deletedProduct: { current: false },
    setPendingOperation: (value) => {
      state.pending = value;
    },
    onSaveProduct: () => {
      throw new Error('Lifecycle must not save the editor document');
    },
    onChangeStatus: async (...args) => {
      writes.push(args);
      return { status: 'confirmed', value: { id: 'product', revision: 8 } };
    },
    setStatus: (value) => {
      state.status = value;
    },
    setDocumentRevision: (value) => {
      state.revision = value;
    },
    setIsSaving: (value) => {
      state.saving = value;
    },
    setErrorMessage: (value) => {
      state.error = value;
    },
    setSuccessMessage: (value) => {
      state.success = value;
    },
    ...overrides
  };
  context.syncRecoveryState = () => {
    state.pending = context.writeAttempt.current.pendingOperation;
  };
  const run = vm.runInNewContext(
    stripTypeScriptTypes(statusHandler) + '\nhandleStatusChange;',
    context
  );
  return { run, state, writes };
}

test('lifecycle action sends only identity, status and predecessor revision', async () => {
  let captured;
  const paths = [];
  const opts = { token: 'token', tenantSlug: 'tenant' };
  const action = vm.runInNewContext(
    stripTypeScriptTypes(
      statusAction.replace('export async function', 'async function')
    ) + '\nchangeProductStatusAction;',
    {
      getProductWriteAuthOpts: async () => opts,
      productWriteOutcome: async (write) => ({
        status: 'confirmed',
        value: await write()
      }),
      refreshProductAfterWrite: async (result) => {
        assert.equal(result.status, 'confirmed');
        paths.push(
          '/dashboard/product',
          `/dashboard/product/${result.value.id}`
        );
      },
      updateProductDetail: async (auth, id, input, key) => {
        assert.equal(key, 'stable-status-key');
        assert.equal(auth, opts);
        captured = { id, input: JSON.parse(JSON.stringify(input)) };
        return { id, revision: 8 };
      },
      revalidatePath: (path) => paths.push(path)
    }
  );
  const result = await action('product', 'ACTIVE', 7, 'stable-status-key', {
    tenantId: 'tenant',
    actorId: 'actor'
  });
  assert.deepEqual(captured, {
    id: 'product',
    input: { status: 'ACTIVE', revision: 7 }
  });
  assert.equal(result.status, 'confirmed');
  assert.equal(result.value.revision, 8);
  assert.deepEqual(paths, ['/dashboard/product', '/dashboard/product/product']);
});

test('status handler does not save dirty document fields and advances the acknowledged revision', async () => {
  const { run, state, writes } = loadHandler();
  await run('ACTIVE');
  assert.equal(writes.length, 1);
  assert.deepEqual(writes[0].slice(0, 3), ['product', 'ACTIVE', 7]);
  assert.match(writes[0][3], /^[a-f0-9-]{36}$/);
  assert.deepEqual(JSON.parse(JSON.stringify(writes[0][4])), {
    tenantId: 'tenant',
    actorId: 'actor'
  });
  assert.equal(state.status, 'ACTIVE');
  assert.equal(state.revision, 8);
  assert.equal(state.saving, false);
});

test('owner refusal does not show an uncommitted status or advance revision', async () => {
  const { run, state } = loadHandler({
    onChangeStatus: async () => {
      throw new Error('Revision conflict');
    }
  });
  await run('ACTIVE');
  assert.equal(state.status, 'DRAFT');
  assert.equal(state.revision, 7);
  assert.equal(state.error, UNKNOWN_PRODUCT_WRITE);
  assert.equal(state.pending, 'status');
  assert.equal(state.success, null);
  assert.equal(state.saving, false);
});

test('status stays unchanged until the owner response arrives', async () => {
  let resolve;
  const pending = new Promise((done) => {
    resolve = done;
  });
  const { run, state } = loadHandler({ onChangeStatus: () => pending });
  const request = run('ACTIVE');
  assert.equal(state.status, 'DRAFT');
  assert.equal(state.saving, true);
  resolve({ status: 'confirmed', value: { id: 'product', revision: 8 } });
  await request;
  assert.equal(state.status, 'ACTIVE');
});

for (const [name, overrides] of [
  ['busy', { isSaving: true }],
  ['new', { isNew: true }],
  ['missing owner callback', { onChangeStatus: undefined }]
]) {
  test(`status handler refuses a ${name} editor`, async () => {
    const { run, writes } = loadHandler(overrides);
    await run('ACTIVE');
    assert.equal(writes.length, 0);
  });
}

test('typed owner refusal leaves displayed status unchanged and allows a corrected attempt', async () => {
  const { run, state } = loadHandler({
    onChangeStatus: async () => ({
      status: 'rejected',
      code: 'PRODUCT_REVISION_CONFLICT',
      message: 'Reload the product'
    })
  });
  await run('ACTIVE');
  assert.equal(state.status, 'DRAFT');
  assert.equal(state.error, 'Reload the product');
  assert.equal(state.pending, null);
});

test('editor initialization restores recovery before enabling writes, including settled and unavailable states', async () => {
  const source = editor.slice(
    editor.indexOf('  const initializeRecovery ='),
    editor.indexOf('\n  React.useEffect(() => {\n    initializeRecovery();')
  );
  assert.ok(
    source.includes('browserProductWriteJournal(writeScope, resourceId)')
  );
  const storage = { text: null, locked: false };
  const journal = () => memoryJournal('product', storage);
  const retained = new ProductWriteAttempt(journal());
  await retained.execute(
    'delete',
    { id: 'product', scope: testScope },
    async () => ({ status: 'unknown', message: 'Lost response' })
  );
  const state = {};
  let factories = 0;
  const context = {
    React: { useCallback: (fn) => fn },
    writeAttempt: { current: null },
    ProductWriteAttempt,
    browserProductWriteJournal: () => {
      factories++;
      return journal();
    },
    writeScope: testScope,
    resourceId: 'product',
    recoveryOnly: true,
    PRODUCT_RECOVERY_UNAVAILABLE: 'Recovery unavailable',
    Error,
    setPendingOperation: (value) => {
      state.pending = value;
    },
    setSettledWrite: (value) => {
      state.settled = value;
    },
    setRecoveryError: (value) => {
      state.error = value;
    },
    setRecoveryReady: (value) => {
      state.ready = value;
    }
  };
  const initialize = vm.runInNewContext(
    stripTypeScriptTypes(source) + '\ninitializeRecovery;',
    context
  );
  initialize();
  assert.equal(state.ready, true);
  assert.equal(state.pending, 'delete');
  assert.equal(factories, 1);
  await retained.resume({
    delete: async () => ({
      status: 'confirmed',
      value: { id: 'product', revision: null }
    })
  });
  initialize();
  assert.equal(state.pending, null);
  assert.equal(state.settled.phase, 'settled');
  assert.equal(factories, 1);
  storage.text = null;
  initialize();
  assert.equal(state.ready, false);
  assert.equal(state.error, 'Recovery unavailable');
  // An unavailable Product with no saved operation must never expose a new draft.
  context.writeAttempt.current = null;
  initialize();
  assert.equal(state.ready, false);
  assert.equal(state.error, 'Recovery unavailable');
});

test('all host editor mounts bind the route resource, and a missing Product exposes recovery only', () => {
  const route = readFileSync(
    new URL(
      '../../../src/app/dashboard/product/[productId]/page.tsx',
      import.meta.url
    ),
    'utf8'
  );
  assert.equal((route.match(/<ProductEditorPage\b/g) || []).length, 3);
  assert.equal((route.match(/key=\{editorKey\}/g) || []).length, 3);
  assert.match(route, /productId,\s+editorIntent \?\? null/);
  assert.equal((route.match(/resourceId=\{productId\}/g) || []).length, 3);
  const missing = route.slice(
    route.indexOf('  if (!product) {'),
    route.indexOf('  // Pre-load effective form')
  );
  assert.match(missing, /isProductEditorIntent\(editorIntent\)/);
  assert.match(missing, /\brecoveryOnly\b/);
  assert.match(missing, /onDeleteProduct=\{deleteProductAction\}/);
});
