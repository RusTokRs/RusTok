/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { randomUUID } from 'node:crypto';
import { stripTypeScriptTypes } from 'node:module';
import vm from 'node:vm';
import { UNKNOWN_PRODUCT_WRITE } from './write-attempt.ts';

const read = (path) => readFileSync(new URL(path, import.meta.url), 'utf8');

const transport = read('../../../src/shared/api/graphql.ts');
const GraphqlError = vm.runInNewContext(
  stripTypeScriptTypes(
    transport.slice(
      transport.indexOf('export class GraphqlError'),
      transport.indexOf('/** Read the host-resolved')
    )
  ).replace('export class', 'class') + '\nGraphqlError;',
  { Error }
);

const executable = (source) =>
  stripTypeScriptTypes(
    source.replace(/^import[\s\S]*?;\s*/gm, '').replace(/\bexport /g, '')
  );

// Exercise the real predicates, not a duplicate implementation.
const { isDefinitiveProductWriteRefusal, productWriteOutcome } =
  vm.runInNewContext(
    executable(read('./api/write-outcome.ts')) +
      '\n({ isDefinitiveProductWriteRefusal, productWriteOutcome });',
    { GraphqlError, UNKNOWN_PRODUCT_WRITE }
  );

const refused = (code = 'PRODUCT_OPERATION_FAILED') =>
  new GraphqlError('Owner refused the write.', code, false, 'rejected');

test('isDefinitiveProductWriteRefusal admits only owner-rejected GraphqlErrors', () => {
  assert.equal(isDefinitiveProductWriteRefusal(refused()), true);
  assert.equal(
    isDefinitiveProductWriteRefusal(refused('IDEMPOTENCY_KEY_CONFLICT')),
    true,
    'a receipt bound to a different payload must rotate the intent key'
  );
  assert.equal(
    isDefinitiveProductWriteRefusal(
      new GraphqlError('Transient', 'PRODUCT_OPERATION_FAILED', true, 'unknown')
    ),
    false,
    'retryable transport failures stay ambiguous so the receipt can replay'
  );
  assert.equal(
    isDefinitiveProductWriteRefusal(
      new GraphqlError('Unknown', 'PRODUCT_OPERATION_FAILED', false, 'unknown')
    ),
    false,
    'unknown outcomes stay ambiguous so the receipt can replay'
  );
  assert.equal(isDefinitiveProductWriteRefusal(new Error('plain')), false);
  assert.equal(isDefinitiveProductWriteRefusal('conflict'), false);
  assert.equal(isDefinitiveProductWriteRefusal(null), false);
});

test('productWriteOutcome maps the secondary-mutation refusal codes', async () => {
  const operationFailed = await productWriteOutcome(() =>
    Promise.reject(refused('PRODUCT_OPERATION_FAILED'))
  );
  assert.equal(operationFailed.status, 'rejected');
  assert.equal(operationFailed.code, 'PRODUCT_OPERATION_FAILED');
  assert.ok(operationFailed.message);

  const keyConflict = await productWriteOutcome(() =>
    Promise.reject(refused('IDEMPOTENCY_KEY_CONFLICT'))
  );
  assert.equal(keyConflict.status, 'rejected');
  assert.equal(keyConflict.code, 'IDEMPOTENCY_KEY_CONFLICT');
  assert.ok(keyConflict.message);

  const unknown = await productWriteOutcome(() =>
    Promise.reject(new Error('socket hang up'))
  );
  assert.equal(unknown.status, 'unknown');
  assert.equal(unknown.message, UNKNOWN_PRODUCT_WRITE);
});

// The relation dialog handler is extracted from the real component source and
// executed with intercepted owner IO; the intent key lifecycle is the contract.
const relationDialog = read(
  './components/products/product-relation-add-dialog.tsx'
);

function relationSubmitter(owner, overrides = {}) {
  const state = {
    keys: [],
    inputs: [],
    messages: [],
    closedWith: [],
    submitting: [],
    attemptKey: { current: null }
  };
  const context = {
    // Align `err instanceof Error` with the realm that builds the failures.
    Error,
    isDefinitiveProductWriteRefusal,
    crypto: { randomUUID },
    onAddRelation: async (input, idempotencyKey) => {
      state.inputs.push(input);
      state.keys.push(idempotencyKey);
      await owner(input, idempotencyKey);
    },
    setIsSubmitting: (value) => state.submitting.push(value),
    setErrorMessage: (message) => state.messages.push(message),
    onOpenChange: (open) => state.closedWith.push(open),
    resetAddAttempt: () => {
      state.attemptKey.current = null;
    },
    addAttemptKey: state.attemptKey,
    selectedProduct: { id: 'related-1' },
    productId: 'product-1',
    relationType: 'CROSS_SELL',
    positionInput: overrides.positionInput ?? '2'
  };
  const handleSubmit = vm.runInNewContext(
    stripTypeScriptTypes(
      relationDialog.slice(
        relationDialog.indexOf('  const handleSubmit = async'),
        relationDialog.indexOf(
          '\n  return (',
          relationDialog.indexOf('  const handleSubmit = async')
        )
      )
    ) + '\nhandleSubmit;',
    context
  );
  return { state, run: () => handleSubmit({ preventDefault: () => {} }) };
}

test('relation dialog retries an ambiguous failure with the same intent key', async () => {
  const { state, run } = relationSubmitter(async () => {
    throw new Error('network dropped mid-flight');
  });
  await run();
  await run();
  assert.equal(state.keys.length, 2);
  assert.equal(
    state.keys[0],
    state.keys[1],
    'an ambiguous retry must replay the same intent, not duplicate the relation'
  );
  assert.ok(state.messages.at(-1), 'the failure is still surfaced');
  assert.deepEqual(state.closedWith, [], 'the dialog stays open for recovery');
  assert.deepEqual(state.inputs[0], {
    productId: 'product-1',
    relatedProductId: 'related-1',
    relationType: 'CROSS_SELL',
    position: 2
  });
});

test('relation dialog rotates the intent key after a definitive refusal', async () => {
  const { state, run } = relationSubmitter(async () => {
    throw refused('PRODUCT_OPERATION_FAILED');
  });
  await run();
  await run();
  assert.equal(state.keys.length, 2);
  assert.notEqual(
    state.keys[0],
    state.keys[1],
    'a definitive refusal must never be replayed with the old key'
  );
  assert.ok(state.messages.at(-1));
  assert.deepEqual(state.closedWith, []);
});

test('relation dialog resets the intent key and closes after confirmation', async () => {
  const { state, run } = relationSubmitter(async () => {});
  await run();
  assert.equal(state.keys.length, 1);
  assert.equal(state.attemptKey.current, null);
  assert.deepEqual(state.closedWith, [false]);
  assert.equal(state.messages.at(-1), null);
});

test('relation dialog rejects an unparsable position without calling the owner', async () => {
  const { state, run } = relationSubmitter(async () => {}, {
    positionInput: ''
  });
  await run();
  assert.deepEqual(
    state.keys,
    [],
    'an invalid position must never reach the owner, let alone mint an intent key'
  );
  assert.equal(
    state.messages.at(-1),
    'Position must be a non-negative whole number.'
  );
  assert.deepEqual(state.closedWith, []);
});

test('numeric fields in secondary dialogs fail closed instead of coercing', () => {
  const relation = read('./components/products/product-relation-add-dialog.tsx');
  assert.ok(
    relation.includes(
      'setErrorMessage(\'Position must be a non-negative whole number.\');'
    ),
    'relation dialog must refuse unparsable positions'
  );

  const bundleItem = read(
    './components/products/product-bundle-item-dialog.tsx'
  );
  assert.ok(
    bundleItem.includes(
      "toast.error('Quantity must be a whole number of at least 1.');"
    ),
    'bundle item dialog must refuse unparsable quantities'
  );
  assert.ok(
    bundleItem.includes(
      "toast.error('Item discount must be between 0 and 100.');"
    ),
    'bundle item dialog must refuse unparsable discounts'
  );
  assert.ok(
    !bundleItem.includes('parseFloat(discountPercent) / 100).toFixed(4)\n        : undefined;'),
    'the discount rate must be validated, not coerced to "NaN"'
  );

  const options = read('./components/attributes/attribute-options-dialog.tsx');
  assert.ok(
    options.includes(
      "toast.error('Position must be a non-negative whole number');"
    ),
    'attribute options dialog must refuse unparsable positions'
  );
  assert.ok(
    !options.includes('Number(e.target.value) || 0'),
    'attribute options dialog must not coerce half-typed values to 0'
  );

  const schema = read('./components/attributes/schema-authoring-card.tsx');
  assert.equal(
    (schema.match(/parsePosition\(/g) ?? []).length,
    4,
    'all four schema authoring position fields must parse fail-closed'
  );
  assert.ok(
    !schema.includes('Number.isNaN(parsed) ? 0 : parsed'),
    'schema authoring must not coerce unparsable positions to 0'
  );
});

// Structural contracts: every secondary ADD surface must mint one intent key,
// keep it across ambiguous retries, rotate it on confirmation or definitive
// refusal, and hand it to the owner call alongside the input.
const secondaryContracts = [
  {
    label: 'media card add-image intent',
    file: './components/products/product-media-card.tsx',
    mint: 'addAttemptKey.current ??= crypto.randomUUID();',
    rotate: 'if (isDefinitiveProductWriteRefusal(err)) resetAddAttempt();',
    resetFn: 'resetAddAttempt',
    resetCalls: 4,
    ownerCall: /await onAddImage\(\s*\{[\s\S]*?\},\s*addAttemptKey\.current\s*\);/
  },
  {
    label: 'relation add dialog intent',
    file: './components/products/product-relation-add-dialog.tsx',
    mint: 'addAttemptKey.current ??= crypto.randomUUID();',
    rotate: 'if (isDefinitiveProductWriteRefusal(err)) resetAddAttempt();',
    resetFn: 'resetAddAttempt',
    resetCalls: 4,
    ownerCall: /await onAddRelation\(\s*\{[\s\S]*?\},\s*addAttemptKey\.current\s*\);/
  },
  {
    label: 'bundle create dialog intent',
    file: './components/bundles/bundle-create-dialog.tsx',
    mint: 'createAttemptKey.current ??= crypto.randomUUID();',
    rotate: 'if (isDefinitiveProductWriteRefusal(err)) resetCreateAttempt();',
    resetFn: 'resetCreateAttempt',
    resetCalls: 4,
    ownerCall: /await onCreate\(\s*\{[\s\S]*?\},\s*createAttemptKey\.current\s*\);/
  },
  {
    label: 'bundle item dialog intent',
    file: './components/products/product-bundle-item-dialog.tsx',
    mint: 'addAttemptKey.current ??= crypto.randomUUID();',
    rotate: 'if (isDefinitiveProductWriteRefusal(err)) resetAddAttempt();',
    resetFn: 'resetAddAttempt',
    resetCalls: 4,
    ownerCall: /await onAdd\(\s*\{[\s\S]*?\},\s*addAttemptKey\.current\s*\);/
  }
];

test('secondary ADD dialogs mint, keep, and rotate one intent key', () => {
  for (const contract of secondaryContracts) {
    const source = read(contract.file);
    assert.ok(
      source.includes(
        "import { isDefinitiveProductWriteRefusal } from '../../api/write-outcome';"
      ),
      `${contract.label}: must classify failures with the shared predicate`
    );
    assert.ok(
      source.includes(contract.mint),
      `${contract.label}: must mint the intent key lazily per attempt`
    );
    assert.ok(
      contract.ownerCall.test(source),
      `${contract.label}: must pass the intent key to the owner call`
    );
    assert.ok(
      source.includes(contract.rotate),
      `${contract.label}: must rotate the key on a definitive refusal`
    );
    const resetCall = `${contract.resetFn}();`;
    const resets = source.split(resetCall).length - 1;
    assert.ok(
      resets >= contract.resetCalls,
      `${contract.label}: expected at least ${contract.resetCalls} reset call sites (success, refusal, reopen, field edits), found ${resets}`
    );
  }
});

test('bundle card init intent is keyed and the item dialog key is forwarded', () => {
  const source = read('./components/products/product-bundle-card.tsx');
  assert.ok(
    source.includes(
      'initBundleAttemptKey.current ??= crypto.randomUUID();'
    ),
    'bundle creation from the card must be keyed per intent'
  );
  assert.equal(
    (source.match(/initBundleAttemptKey\.current = null;/g) ?? []).length,
    2,
    'the init key must rotate on confirmation and on definitive refusal'
  );
  assert.ok(
    source.includes('await onAddBundleItem(input, idempotencyKey);'),
    'the item dialog intent key must be forwarded to the host action'
  );
  assert.ok(
    source.includes('isDefinitiveProductWriteRefusal'),
    'the init failure path must classify definitive refusals'
  );
});

test('API and server actions forward the caller intent key', () => {
  const products = read('./api/products.ts');
  const addProductImage = products.slice(
    products.indexOf('export async function addProductImage('),
    products.indexOf('export async function deleteProductImage(')
  );
  assert.ok(
    /idempotencyKey = crypto\.randomUUID\(\)/.test(addProductImage),
    'addProductImage must accept a caller key and default one when absent'
  );
  assert.ok(
    !addProductImage.includes('const idempotencyKey = crypto.randomUUID();'),
    'addProductImage must not silently replace the caller intent key'
  );

  const actions = read(
    '../../../src/app/dashboard/product/actions.ts'
  );
  assert.equal(
    (actions.match(/idempotencyKey\?: string/g) ?? []).length,
    4,
    'the four secondary ADD actions must accept an optional intent key'
  );
  assert.ok(actions.includes('addProductRelation(opts, input, idempotencyKey)'));
  assert.ok(actions.includes('createBundle(opts, input, locale, idempotencyKey)'));
  assert.ok(actions.includes('addBundleItem(opts, input, idempotencyKey)'));
  assert.ok(
    /addProductImage\(\s*opts,\s*productId,\s*\{[\s\S]*?\},\s*idempotencyKey\s*\);/.test(
      actions
    ),
    'addImageAction must forward the intent key to the API layer'
  );
});

test('host pages forward the intent key from the dialogs to the actions', () => {
  const editorHost = read(
    '../../../src/app/dashboard/product/[productId]/page.tsx'
  );
  assert.ok(
    editorHost.includes('addImageAction(pid, img, idempotencyKey)'),
    'editor host must forward the media intent key'
  );
  assert.ok(
    editorHost.includes('addProductRelationAction(input, idempotencyKey)'),
    'editor host must forward the relation intent key'
  );
  assert.ok(
    editorHost.includes('createBundleAction(input, undefined, idempotencyKey)'),
    'editor host must forward the bundle intent key past the locale slot'
  );
  assert.ok(
    editorHost.includes('addBundleItemAction(productId, input, idempotencyKey)'),
    'editor host must forward the bundle item intent key'
  );

  const relationsHost = read(
    '../../../src/app/dashboard/product-relations/page.tsx'
  );
  assert.ok(
    relationsHost.includes('addProductRelationAction(input, idempotencyKey)'),
    'relations host must forward the intent key'
  );

  const bundlesHost = read(
    '../../../src/app/dashboard/product/bundles/page.tsx'
  );
  assert.ok(
    bundlesHost.includes('createBundleAction(input, undefined, idempotencyKey)'),
    'bundles host must forward the intent key past the locale slot'
  );

  const editorPage = read('./pages/product-editor-page.tsx');
  for (const forward of [
    'onAddRelation(input, idempotencyKey)',
    'onCreateBundle(input, idempotencyKey)',
    'onAddBundleItem(input, idempotencyKey)'
  ]) {
    assert.ok(
      editorPage.includes(`await ${forward};`),
      `editor page must forward the intent key: ${forward}`
    );
  }
  assert.ok(
    /await onAddImage\(\s*initialProduct\.id,[\s\S]*?\{ \.\.\.img, locale: activeLocale \},\s*idempotencyKey\s*\);/.test(
      editorPage
    ),
    'editor page must forward the media intent key with the active content locale'
  );

  const bundlesPage = read('./pages/bundles-page.tsx');
  assert.ok(
    bundlesPage.includes('onCreateBundle(input, idempotencyKey)'),
    'bundles page must forward the intent key'
  );
});
