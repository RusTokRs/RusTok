/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
// Source coverage only until the maintainer runs it; no mounted-client evidence is implied.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import vm from 'node:vm';
import { ProductWriteAttempt, UNKNOWN_PRODUCT_WRITE } from './write-attempt.ts';
import {
  browserAttributeDefinitionJournal,
  ATTRIBUTE_DEFINITION_INTENT_QUERY_KEY
} from './write-journal.ts';
import { memoryJournal } from './write-test-support.mjs';
import {
  attributePresentationJson,
  prepareAttributeDefinition,
  isAttributeDefinitionInput,
  isAttributeDefinitionRequest,
  isAttributeDefinitionResult,
  isAttributeLifecycleRequest,
  isAttributeWorkspace,
  isDefinitionId,
  isDefinitionLocale,
  attributeDefinitionRefusalCode
} from './attribute-lifecycle.ts';

const id = (n) => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`;
const scope = { tenantId: id(90), actorId: id(91) };
const unknown = { status: 'unknown', message: 'Response lost' };
const state = () => ({ text: null, locked: false });
const journal = (storage = state(), expectedScope = scope) =>
  memoryJournal(
    id(1),
    storage,
    expectedScope,
    undefined,
    'attribute_definition'
  );
const workspace = () => ({
  attributes: [
    {
      id: id(1),
      code: 'color',
      valueType: 'select',
      scope: 'both',
      isLocalized: false,
      isFilterable: false,
      isSearchable: false,
      isSortable: false,
      isComparable: false,
      showOnStorefront: true,
      showInAdminGrid: false,
      searchWeight: 0,
      position: 0,
      isArchived: false,
      translation: null,
      stateToken: 'a'.repeat(64)
    }
  ],
  options: [
    {
      id: id(2),
      attributeId: id(1),
      code: 'red',
      position: 0,
      isArchived: false,
      attributeArchived: false,
      label: null,
      stateToken: 'b'.repeat(64)
    }
  ],
  impact: { references: [], hasMore: false, inspectionComplete: true }
});
const request = (optionId = id(2), action = 'archive', json = '') =>
  prepareAttributeDefinition(scope, workspace(), optionId, action, 'ru', json);
const decision = (input, refusal = null) => ({
  attributeId: input.attributeId,
  optionId: input.optionId,
  applied: refusal === null,
  stateToken: refusal === 'not_found' ? null : 'c'.repeat(64),
  refusal,
  impact:
    refusal === 'in_use'
      ? {
          references: [{ kind: 'allowed_option', resourceId: id(8) }],
          hasMore: false,
          inspectionComplete: true
        }
      : refusal === 'inspection_limit'
        ? { references: [], hasMore: true, inspectionComplete: false }
        : null
});
const confirmed = (value = request()) => ({
  status: 'confirmed',
  value: { id: value.id, revision: null, definition: decision(value.input) }
});
const rejected = (value, refusal) => ({
  status: 'rejected',
  code: attributeDefinitionRefusalCode(refusal),
  message: 'Owner refused',
  definition: decision(value.input, refusal)
});

// Exercise the actual adapters with intercepted IO, not a duplicate SDK implementation.
const executable = (source) =>
  stripTypeScriptTypes(
    source.replace(/^import[\s\S]*?;\s*/gm, '').replace(/\bexport /g, '')
  );
class OwnerError extends Error {
  constructor(code, retryable, outcome) {
    super('Owner error');
    Object.assign(this, { code, retryable, outcome });
  }
}
function sdk() {
  const productWriteOutcome = vm.runInNewContext(
    executable(
      readFileSync(new URL('./api/write-outcome.ts', import.meta.url), 'utf8')
    ) + '\nproductWriteOutcome;',
    { GraphqlError: OwnerError, UNKNOWN_PRODUCT_WRITE }
  );
  return vm.runInNewContext(
    executable(
      readFileSync(
        new URL('./api/attribute-lifecycle.ts', import.meta.url),
        'utf8'
      )
    ) + '\n({ readAttributeLifecycle, updateAttributeDefinition });',
    {
      isAttributeDefinitionRequest,
      isAttributeDefinitionResult,
      isAttributeLifecycleRequest,
      isAttributeWorkspace,
      isDefinitionId,
      isDefinitionLocale,
      attributeDefinitionRefusalCode,
      productWriteOutcome,
      UNKNOWN_PRODUCT_WRITE,
      structuredClone,
      graphqlRequest: () =>
        assert.fail('the injected executor must intercept IO')
    }
  );
}

test('presentation preparation materializes exact copy once and cannot mutate identity/storage shape', () => {
  const value = request(
    null,
    'update',
    '{"translation":{"label":"  Цвет  "},"isFilterable":true,"position":0}'
  );
  assert.deepEqual(value.input.attributePatch.translation, {
    label: '  Цвет  ',
    helpText: null,
    facetLabel: null,
    seoLabel: null
  });
  assert.equal(value.input.optionPatch, null);
  for (const json of [
    '{"code":"other"}',
    '{"valueType":"text"}',
    '{"defaultValue":"red"}',
    '{"validation":{}}',
    '{"metadata":{}}',
    '{"translation":[]}',
    '[]',
    '{"position":-1}',
    '{"searchWeight":2147483648}',
    '{"translation":{"label":""}}'
  ])
    assert.throws(() => request(null, 'update', json));
  for (const value of ['\u0085', '\ud800', '\udc00']) {
    assert.throws(() =>
      request(null, 'update', JSON.stringify({ translation: { label: value } }))
    );
    assert.throws(() =>
      request(id(2), 'update', JSON.stringify({ label: value }))
    );
  }
  assert.throws(() =>
    request(
      null,
      'update',
      JSON.stringify({ translation: { label: 'Valid', helpText: '\ud800' } })
    )
  );
  assert.equal(
    request(id(2), 'update', JSON.stringify({ label: '😀' })).input.optionPatch
      .label,
    '😀'
  );
  assert.equal(
    request(id(2), 'update', JSON.stringify({ label: '\ufeff' })).input
      .optionPatch.label,
    '\ufeff'
  );
  assert.throws(() => request(id(2), 'update', '{"id":"replacement"}'));
  assert.throws(() => request(null, 'update', ' '.repeat(8193)));
  assert.throws(() => request(null, 'update', 'я'.repeat(4097)));
  assert.equal(
    request(null, 'archive', '{invalid JSON').input.attributePatch,
    null
  );
  assert.equal(
    request(id(2), 'restore', ' '.repeat(8193)).input.optionPatch,
    null
  );
  assert.equal(
    isAttributeLifecycleRequest({
      attributeId: null,
      optionId: null,
      includeArchived: true,
      tenantId: id(99)
    }),
    false
  );
  assert.equal(
    isAttributeLifecycleRequest({
      attributeId: null,
      optionId: id(2),
      includeArchived: true
    }),
    false
  );
  for (const locale of ['', 'und', 'EN', 'en_US', 'en-u-ca-hebrew']) {
    const changed = structuredClone(value.input);
    changed.locale = locale;
    assert.equal(isAttributeDefinitionInput(changed), false);
  }
  const changed = structuredClone(value.input);
  changed.attributeId = '00000000-0000-0000-0000-000000000000';
  assert.equal(isAttributeDefinitionInput(changed), false);
});

test('historical projections remain readable but missing option UUIDs never select the parent', () => {
  const value = workspace();
  value.attributes[0].position = -1;
  value.attributes[0].searchWeight = -2;
  value.attributes[0].translation = {
    label: '',
    helpText: null,
    facetLabel: null,
    seoLabel: null
  };
  value.options[0].position = -3;
  value.options[0].label = '';
  assert.equal(isAttributeWorkspace(value), true);
  assert.equal(attributePresentationJson(value, id(9)), null);
  assert.throws(() =>
    prepareAttributeDefinition(scope, value, id(9), 'archive', 'ru', '')
  );
  const duplicate = workspace();
  duplicate.options.push(duplicate.options[0]);
  assert.equal(isAttributeWorkspace(duplicate), false);
  const foreign = workspace();
  foreign.options[0].attributeId = id(99);
  assert.throws(() =>
    prepareAttributeDefinition(
      scope,
      foreign,
      id(2),
      'update',
      'ru',
      '{"position":1}'
    )
  );
});

test('unknown freezes complete scope/body/key/locale before IO and reload recovers without any GET', async () => {
  const storage = state();
  const original = request(
    null,
    'update',
    '{"translation":{"label":"Исходное"},"isSearchable":true}'
  );
  const first = new ProductWriteAttempt(journal(storage));
  let sent;
  await first.execute('definition', original, async (body, key) => {
    const durable = JSON.parse(storage.text).record;
    assert.deepEqual(durable.attempt.request, body);
    assert.equal(durable.attempt.key, key);
    assert.ok(Object.isFrozen(body.input.attributePatch.translation));
    sent = { body, key };
    return unknown;
  });
  original.input.locale = 'en';
  original.input.attributePatch.translation.label = 'Changed draft';
  original.scope.actorId = id(99);
  const reloaded = new ProductWriteAttempt(journal(storage));
  const result = await reloaded.resume({
    definition: async (body, key) => {
      assert.deepEqual({ body, key }, sent);
      return confirmed(body);
    }
  });
  assert.equal(result.outcome.status, 'confirmed');
  assert.equal(reloaded.recovery.operation, 'definition');
  assert.deepEqual(reloaded.recovery.request, sent.body);
  assert.equal(reloaded.recovery.request.input.locale, 'ru');
});

test('dictionary namespaces and foreign actor/attribute scope fence other operations before IO', async () => {
  const storage = state();
  const writer = new ProductWriteAttempt(journal(storage));
  await writer.execute('definition', request(), async () => unknown);
  assert.throws(() => journal(storage, { ...scope, actorId: id(92) }).read());
  assert.throws(() =>
    memoryJournal(
      id(3),
      storage,
      scope,
      undefined,
      'attribute_definition'
    ).read()
  );
  assert.throws(() => memoryJournal(id(1), storage, scope).read());
  let sends = 0;
  const result = await writer.execute(
    'delete',
    { scope, id: id(1) },
    async () => {
      sends++;
      return confirmed();
    }
  );
  assert.equal(result.outcome.status, 'unknown');
  assert.equal(sends, 0);
  // Supplying a different draft cannot retarget the existing pending command.
  await writer.execute('definition', request(null), async (body) => {
    assert.equal(body.input.optionId, id(2));
    sends++;
    return unknown;
  });
  assert.equal(sends, 1);
});

for (const refusal of [
  'stale',
  'in_use',
  'inspection_limit',
  'not_found',
  'parent_archived'
]) {
  test(`${refusal}: terminal owner decision retains full request and cannot implicitly mint another key`, async () => {
    const storage = state();
    const original = request(
      id(2),
      refusal === 'parent_archived' ? 'restore' : 'archive'
    );
    const writer = new ProductWriteAttempt(journal(storage));
    await writer.execute('definition', original, async () =>
      rejected(original, refusal)
    );
    assert.equal(writer.recovery.phase, 'settled');
    assert.deepEqual(writer.recovery.request, original);
    assert.equal(writer.recovery.outcome.definition.refusal, refusal);
    const restored = new ProductWriteAttempt(journal(storage));
    assert.equal(
      await restored.execute('definition', request(), async () =>
        assert.fail('fresh intent is required')
      ),
      null
    );
    assert.equal(
      await restored.resume({
        definition: async () => assert.fail('terminal is not pending')
      }),
      null
    );
  });
}

test('wrong IDs/revisions, incompatible refusals and admission errors remain Unknown', async () => {
  const original = request();
  const badRefusal = rejected(original, 'in_use');
  badRefusal.code = attributeDefinitionRefusalCode('stale');
  for (const outcome of [
    { status: 'confirmed', value: { id: id(1), revision: null } },
    { status: 'confirmed', value: { ...confirmed().value, revision: 1 } },
    { status: 'confirmed', value: { ...confirmed().value, id: id(9) } },
    { status: 'confirmed', value: { ...confirmed().value, variantId: id(9) } },
    {
      status: 'confirmed',
      value: {
        ...confirmed().value,
        definition: { ...decision(original.input), optionId: null }
      }
    },
    { status: 'rejected', code: 'UNAUTHORIZED', message: 'Preflight' },
    {
      status: 'rejected',
      code: attributeDefinitionRefusalCode('not_found'),
      message: 'No receipt evidence'
    },
    badRefusal
  ]) {
    const writer = new ProductWriteAttempt(journal());
    const result = await writer.execute(
      'definition',
      original,
      async () => outcome
    );
    assert.equal(result.outcome.status, 'unknown');
    assert.equal(writer.recovery.phase, 'pending');
  }
  assert.equal(
    isAttributeDefinitionResult(
      decision(original.input, 'parent_archived'),
      original.input
    ),
    false
  );
  assert.equal(
    isAttributeDefinitionResult(
      decision(request(id(2), 'update', '{"position":0}').input, 'in_use'),
      request(id(2), 'update', '{"position":0}').input
    ),
    false
  );
});

for (const damage of ['missing', 'corrupt', 'key', 'body']) {
  test(`late acknowledgement cannot overwrite ${damage} recovery evidence`, async () => {
    const storage = state();
    const writer = new ProductWriteAttempt(journal(storage));
    let damaged;
    const result = await writer.execute('definition', request(), async () => {
      if (damage === 'missing') storage.text = null;
      else if (damage === 'corrupt') storage.text = '{bad';
      else {
        const frame = JSON.parse(storage.text);
        if (damage === 'key') frame.record.attempt.key = id(9);
        else frame.record.attempt.request.input.optionId = null;
        storage.text = JSON.stringify(frame);
      }
      damaged = storage.text;
      return confirmed();
    });
    assert.equal(result.outcome.status, 'unknown');
    assert.equal(writer.pendingOperation, 'definition');
    assert.equal(storage.text, damaged);
    assert.ok(writer.recoveryError);
  });
}

test('actual SDK binds frozen locale/tenant/key and does not treat transport failures as owner decisions', async () => {
  const api = sdk();
  const original = request();
  const calls = [];
  const opts = {
    token: 'test',
    tenantSlug: 'shop',
    tenantId: scope.tenantId,
    locale: 'en',
    graphql: async (...args) => {
      calls.push(args);
      return { updateProductAttributeDefinition: decision(original.input) };
    }
  };
  assert.equal(
    (await api.updateAttributeDefinition(opts, original, id(8))).status,
    'confirmed'
  );
  assert.deepEqual(JSON.parse(JSON.stringify(calls[0][1])), {
    idempotencyKey: id(8),
    input: { ...original.input, action: 'ARCHIVE' }
  });
  assert.deepEqual(JSON.parse(JSON.stringify(calls[0][4])), {
    tenantId: scope.tenantId,
    locale: 'ru'
  });
  assert.equal(
    (
      await api.updateAttributeDefinition(
        { ...opts, tenantId: id(99) },
        original,
        id(8)
      )
    ).status,
    'unknown'
  );
  assert.equal(
    (await api.updateAttributeDefinition(opts, original, 'not-a-key')).status,
    'unknown'
  );
  assert.equal(calls.length, 1);
  for (const error of [
    new Error('Network'),
    new OwnerError('PRODUCT_VALIDATION', undefined, undefined),
    new OwnerError('UNAUTHORIZED', false, 'rejected')
  ]) {
    assert.equal(
      (
        await api.updateAttributeDefinition(
          {
            ...opts,
            graphql: async () => {
              throw error;
            }
          },
          original,
          id(8)
        )
      ).status,
      'unknown'
    );
  }
  assert.equal(
    (
      await api.updateAttributeDefinition(
        {
          ...opts,
          graphql: async () => {
            throw new OwnerError('PRODUCT_VALIDATION', false, 'rejected');
          }
        },
        original,
        id(8)
      )
    ).status,
    'rejected'
  );
});

test('actual selected read accepts absent target without inventing parent impact and rejects foreign/incoherent projections', async () => {
  const api = sdk();
  const missing = workspace();
  missing.options = [];
  missing.impact = null;
  const requested = {
    attributeId: id(1),
    optionId: id(9),
    includeArchived: true
  };
  let calls = 0;
  const opts = {
    token: 'test',
    tenantSlug: 'shop',
    tenantId: scope.tenantId,
    graphql: async (_query, variables, _token, _slug, options) => {
      calls++;
      assert.equal(variables.locale, 'ru');
      assert.equal(options.locale, 'ru');
      assert.equal(options.tenantId, scope.tenantId);
      return { productAttributeLifecycle: missing };
    }
  };
  assert.equal(
    (await api.readAttributeLifecycle(opts, 'ru', requested)).impact,
    null
  );
  assert.equal(calls, 1);
  for (const payload of [
    { ...missing, impact: workspace().impact },
    {
      ...workspace(),
      options: [{ ...workspace().options[0], attributeId: id(99) }]
    },
    {
      ...workspace(),
      options: [{ ...workspace().options[0], attributeArchived: true }]
    }
  ]) {
    await assert.rejects(
      api.readAttributeLifecycle(
        {
          ...opts,
          graphql: async () => ({ productAttributeLifecycle: payload })
        },
        'ru',
        requested
      )
    );
  }
  await assert.rejects(
    api.readAttributeLifecycle(opts, 'ru', { ...requested, tenantId: id(99) })
  );
  assert.equal(
    calls,
    1,
    'a forged request cannot override tenant/locale through a spread'
  );
});

test('actual host action binds actor scope and cache failure never erases acknowledgement', async () => {
  const source = readFileSync(
    new URL(
      '../../../src/app/dashboard/product/attributes/actions.ts',
      import.meta.url
    ),
    'utf8'
  );
  const helper = source.slice(
    source.indexOf('async function getSessionOpts('),
    source.indexOf('export async function createAttributeAction(')
  );
  const action = source.slice(
    source.indexOf('export async function updateAttributeDefinitionAction(')
  );
  let actorId = scope.actorId;
  const calls = [];
  const run = vm.runInNewContext(
    stripTypeScriptTypes((helper + action).replace('export async', 'async')) +
      '\nupdateAttributeDefinitionAction;',
    {
      auth: async () => ({
        user: {
          id: actorId,
          tenantId: scope.tenantId,
          tenantSlug: 'shop',
          rustokToken: 'test'
        }
      }),
      graphqlRequest: () => assert.fail('the SDK must own IO'),
      isAttributeDefinitionRequest,
      updateAttributeDefinition: async (opts, body, key) => {
        calls.push({ opts, body, key });
        return confirmed(body);
      },
      revalidatePath: () => {
        throw new Error('Cache unavailable after commit');
      }
    }
  );
  const original = request();
  assert.equal((await run(original, id(8))).status, 'confirmed');
  assert.equal(calls[0].body, original);
  assert.equal(calls[0].key, id(8));
  actorId = id(99);
  assert.equal((await run(original, id(8))).status, 'unknown');
  assert.equal(calls.length, 1);
});

function browser(t) {
  const previousWindow = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const previousNavigator = Object.getOwnPropertyDescriptor(
    globalThis,
    'navigator'
  );
  const records = new Map();
  const win = {
    isSecureContext: true,
    location: {
      href: `https://admin.example/dashboard/product/attributes?attribute_id=${id(1)}&option_id=${id(2)}`
    },
    localStorage: {
      getItem: (key) => records.get(key) ?? null,
      setItem: (key, value) => records.set(key, value)
    },
    history: {
      state: { router: 'preserved' },
      replaceState(state, _unused, url) {
        this.state = state;
        win.location.href = url.toString();
      }
    }
  };
  Object.defineProperty(globalThis, 'window', {
    configurable: true,
    value: win
  });
  Object.defineProperty(globalThis, 'navigator', {
    configurable: true,
    value: { locks: { request: async (_key, _opts, work) => work({}) } }
  });
  t.after(() => {
    if (previousWindow)
      Object.defineProperty(globalThis, 'window', previousWindow);
    else delete globalThis.window;
    if (previousNavigator)
      Object.defineProperty(globalThis, 'navigator', previousNavigator);
    else delete globalThis.navigator;
  });
  return { win, records };
}

test('browser Fresh requires durable terminal proof, retains old body and keeps selected UUIDs', async (t) => {
  const { win, records } = browser(t);
  const originalJournal = browserAttributeDefinitionJournal(scope, id(1));
  const writer = new ProductWriteAttempt(originalJournal);
  await writer.execute('definition', request(), async () => unknown);
  assert.throws(() => browserAttributeDefinitionJournal(scope, id(1), true));
  await writer.resume({ definition: async (body) => confirmed(body) });
  const terminal = writer.recovery;
  const fresh = browserAttributeDefinitionJournal(scope, id(1), true);
  assert.notEqual(fresh.intent, originalJournal.intent);
  assert.equal(fresh.read(), null);
  assert.deepEqual(originalJournal.read(), terminal);
  assert.equal(records.size, 2);
  const url = new URL(win.location.href);
  assert.equal(url.searchParams.get('attribute_id'), id(1));
  assert.equal(url.searchParams.get('option_id'), id(2));
  assert.equal(
    url.searchParams.get(ATTRIBUTE_DEFINITION_INTENT_QUERY_KEY),
    fresh.intent
  );
  records.clear();
  assert.throws(() => browserAttributeDefinitionJournal(scope, id(1)));
});

for (const damaged of [
  `?attribute_id=${id(1)}&option_id=attribute`,
  `?attribute_id=${id(1)}&option_id=`,
  `?attribute_id=${id(1)}&option_id=${id(0xab).toUpperCase()}`,
  `?attribute_id=${id(1)}&attribute_id=${id(1)}`,
  `?attribute_id=${id(1)}&option_id=${id(2)}&option_id=${id(2)}`,
  `?attribute_id=${id(99)}`,
  `?option_id=${id(2)}`,
  `?attribute_id=%20${id(1)}`
])
  test(`browser raw identity refuses before baseline: ${damaged}`, (t) => {
    const { win, records } = browser(t);
    win.location.href =
      'https://admin.example/dashboard/product/attributes' + damaged;
    assert.throws(() => browserAttributeDefinitionJournal(scope, id(1)));
    assert.equal(records.size, 0);
  });

test('browser rejects a rewritten document after lock acquisition and before IO', async (t) => {
  const { win } = browser(t);
  const journal = browserAttributeDefinitionJournal(scope, id(1), false, id(2));
  const writer = new ProductWriteAttempt(journal);
  writer.restore();
  Object.defineProperty(globalThis, 'navigator', {
    configurable: true,
    value: {
      locks: {
        request: async (_key, _options, work) => {
          const changed = new URL(win.location.href);
          changed.searchParams.set('option_id', id(99));
          win.location.href = changed.toString();
          return work({});
        }
      }
    }
  });
  let calls = 0;
  const blocked = await writer.execute(
    'definition',
    request(),
    async (body) => {
      calls++;
      return confirmed(body);
    }
  );
  assert.equal(blocked.outcome.status, 'unknown');
  assert.equal(calls, 0);
});

test('Fresh publishes retained nullable target before Ready so failed navigation cannot retarget', async (t) => {
  const { win, records } = browser(t);
  const journal = browserAttributeDefinitionJournal(scope, id(1), false, id(2));
  const writer = new ProductWriteAttempt(journal);
  await writer.execute('definition', request(null), async (body) =>
    confirmed(body)
  );
  const original = writer.recovery;
  const fresh = browserAttributeDefinitionJournal(scope, id(1), true, id(2));
  assert.equal(fresh.read(), null);
  assert.equal(new URL(win.location.href).searchParams.get('option_id'), null);
  win.location.assign = () => {
    throw new Error('Navigation unavailable');
  };
  assert.throws(() => win.location.assign(win.location.href));
  const reloaded = browserAttributeDefinitionJournal(scope, id(1), false, null);
  assert.equal(reloaded.intent, fresh.intent);
  assert.equal(reloaded.read(), null);
  const saved = [...records.values()]
    .map((value) => JSON.parse(value))
    .find((value) => value.intent === journal.intent);
  assert.deepEqual(saved.record, original);
});
