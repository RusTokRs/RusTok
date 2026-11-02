/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import {
  seedReconfigurationAxes,
  refreshReconfigurationChoices,
  reconfigurationAxis,
  axesProposal,
  reassignAxisValue,
  isAxesApplicationInput,
  validAxesPreview
} from './axis-reconfiguration.ts';
import { ProductWriteAttempt } from './write-attempt.ts';
import { memoryJournal } from './write-test-support.mjs';

const product = randomUUID(),
  attribute = randomUUID(),
  red = randomUUID(),
  blue = randomUUID(),
  sku = randomUUID();
const scope = { tenantId: randomUUID(), actorId: randomUUID() };
const form = {
  categoryId: randomUUID(),
  detachedAttributeIds: [],
  attributes: [
    {
      attributeId: attribute,
      code: 'color',
      label: 'Color',
      scope: 'variant',
      valueType: 'select',
      isDisabled: false,
      variantAxisPolicy: 'allowed',
      defaultVariantAxis: true,
      options: [
        { id: red, label: 'Red' },
        { id: blue, label: 'Blue' }
      ]
    }
  ]
};
const saved = [
  {
    id: randomUUID(),
    attributeId: attribute,
    code: 'color',
    name: 'Saved color',
    position: 0,
    allowedValues: [{ optionId: red, value: 'Saved red', position: 0 }]
  }
];
const application = () => ({
  axes: [
    { attributeId: attribute, position: 0, allowedOptionIds: [red, blue] }
  ],
  expectedState: 'a'.repeat(64),
  reassignments: [
    { variantId: sku, axisValues: [{ attributeId: attribute, optionId: blue }] }
  ]
});
const request = () => ({
  id: product,
  scope,
  locale: 'en',
  input: application()
});
const preview = () => ({
  productId: product,
  productRevision: 7,
  stateToken: 'a'.repeat(64),
  variantCount: 1,
  canApply: true,
  hasChanges: true,
  conflicts: [],
  hasMoreConflicts: false,
  variants: [
    {
      variantId: sku,
      sku: 'sku',
      currentValues: [{ attributeId: attribute, optionId: red }],
      proposedValues: [{ attributeId: attribute, optionId: blue }],
      changes: true
    }
  ]
});

test('existing-axis seed is saved-only and retains unavailable IDs rather than applying new defaults', () => {
  const unavailable = randomUUID();
  const axes = structuredClone(saved);
  axes[0].allowedValues.push({
    optionId: unavailable,
    value: 'Retained',
    position: 100
  });
  const changedForm = structuredClone(form);
  changedForm.attributes.push({
    ...changedForm.attributes[0],
    attributeId: randomUUID(),
    defaultVariantAxis: true
  });
  const rows = seedReconfigurationAxes(axes, changedForm);
  assert.equal(rows.length, 1);
  assert.deepEqual(rows[0].allowedOptionIds, [red, unavailable]);
  assert.equal(
    rows[0].options.find((option) => option.id === unavailable).available,
    false
  );
  assert.equal(rows[0].allowedOptionIds.includes(blue), false);
  assert.equal(seedReconfigurationAxes([], changedForm).length, 0);
  const dirty = seedReconfigurationAxes(saved, null);
  dirty[0].allowedOptionIds = [blue, unavailable];
  const refreshed = refreshReconfigurationChoices(dirty, form);
  assert.deepEqual(refreshed[0].allowedOptionIds, [blue, unavailable]);
  assert.equal(
    refreshed[0].options.find((option) => option.id === blue).available,
    true
  );
  const largeForm = structuredClone(form);
  largeForm.attributes[0].options.push(
    ...Array.from({ length: 300 }, () => ({ id: randomUUID(), label: 'Extra' }))
  );
  const bounded = seedReconfigurationAxes(saved, largeForm)[0];
  assert.equal(bounded.dictionaryLimited, true);
  assert.deepEqual(
    bounded.options.map((option) => option.id),
    [red]
  );
  assert.equal(
    reconfigurationAxis(attribute, {
      ...form,
      attributes: [
        {
          ...form.attributes[0],
          options: Array.from({ length: 257 }, () => ({
            id: randomUUID(),
            label: 'value'
          }))
        }
      ]
    }),
    null
  );
});

test('full tuple edits keep now-unexpected fields until an explicit reset and proposal takes a snapshot', () => {
  const removed = randomUUID(),
    option = randomUUID();
  const source = preview().variants[0];
  source.proposedValues.push({ attributeId: removed, optionId: option });
  const edits = reassignAxisValue([], source, attribute, red);
  assert.deepEqual(
    edits[0].axisValues.find((value) => value.attributeId === removed),
    { attributeId: removed, optionId: option }
  );
  const input = axesProposal(seedReconfigurationAxes(saved, form), edits);
  edits[0].axisValues.length = 0;
  assert.equal(input.reassignments[0].axisValues.length, 2);
  assert.equal(Object.hasOwn(input, 'expectedState'), false);
});

test('recovery payload whitelist is bounded and never normalizes a persisted tuple', () => {
  const input = application(),
    before = JSON.stringify(input);
  assert.ok(isAxesApplicationInput(input));
  assert.equal(JSON.stringify(input), before);
  for (const change of [
    (value) => {
      delete value.expectedState;
    },
    (value) => {
      value.expectedState = 'A'.repeat(64);
    },
    (value) => {
      value.axes[0].attributeId = '00000000-0000-0000-0000-000000000000';
    },
    (value) => {
      value.axes[0].position = -1;
    },
    (value) => {
      value.axes[0].allowedOptionIds = [];
    },
    (value) => {
      value.axes[0].allowedOptionIds = Array.from({ length: 257 }, () =>
        randomUUID()
      );
    },
    (value) => {
      value.reassignments = Array.from({ length: 257 }, () =>
        structuredClone(value.reassignments[0])
      );
    },
    (value) => {
      value.reassignments[0].credentials = 'not allowed';
    }
  ]) {
    const bad = application();
    change(bad);
    assert.equal(isAxesApplicationInput(bad), false);
  }
});

test('owner preview must match product, complete bounded membership, token and closed conflict codes', () => {
  assert.ok(validAxesPreview(product, preview()));
  for (const change of [
    (value) => {
      value.productId = randomUUID();
    },
    (value) => {
      value.stateToken = 'unknown';
    },
    (value) => {
      value.variantCount = 0;
    },
    (value) => {
      value.variants.push(structuredClone(value.variants[0]));
      value.variantCount = 2;
    },
    (value) => {
      value.conflicts = [
        {
          kind: 'new_unrecognized_kind',
          variantIds: [],
          attributeId: null,
          optionId: null,
          hasMoreVariants: false
        }
      ];
      value.canApply = false;
    },
    (value) => {
      value.hasMoreConflicts = true;
    }
  ]) {
    const bad = preview();
    change(bad);
    assert.equal(validAxesPreview(product, bad), false);
  }
});

test('axes are durable before IO; unknown reload replays exact input, key and frozen locale', async () => {
  const state = { text: null, locked: false },
    initial = request();
  const first = new ProductWriteAttempt(memoryJournal(product, state, scope));
  let sent;
  await first.execute('axes', initial, async (input, key) => {
    const durable = JSON.parse(state.text).record;
    assert.equal(durable.phase, 'pending');
    assert.equal(durable.attempt.key, key);
    assert.deepEqual(durable.attempt.request, initial);
    sent = { input, key };
    return { status: 'unknown', message: 'Response lost' };
  });
  const resumed = new ProductWriteAttempt(memoryJournal(product, state, scope));
  assert.equal(resumed.restore().phase, 'pending');
  const changed = request();
  changed.locale = 'ru';
  changed.input.expectedState = 'b'.repeat(64);
  const result = await resumed.execute('axes', changed, async (input, key) => {
    assert.deepEqual(input, sent.input);
    assert.equal(input.locale, 'en');
    assert.equal(key, sent.key);
    return { status: 'confirmed', value: { id: product, revision: null } };
  });
  assert.equal(result.outcome.status, 'confirmed');
  assert.equal(resumed.pendingOperation, null);
  assert.equal(JSON.parse(state.text).record.phase, 'settled');
  assert.equal(state.text.includes('expectedState'), false);
});

test('storage failure prevents axes POST and a guessed Product revision cannot acknowledge axes', async () => {
  let calls = 0;
  const state = {
    locked: false,
    get text() {
      return null;
    },
    set text(_value) {
      throw new Error('Storage unavailable');
    }
  };
  const blocked = new ProductWriteAttempt(memoryJournal(product, state, scope));
  await blocked.execute('axes', request(), async () => {
    calls++;
    return { status: 'unknown', message: 'must not send' };
  });
  assert.equal(calls, 0);
  assert.ok(blocked.recoveryError);
  const attempt = new ProductWriteAttempt(
    memoryJournal(product, undefined, scope)
  );
  const result = await attempt.execute('axes', request(), async () => ({
    status: 'confirmed',
    value: { id: product, revision: 8 }
  }));
  assert.equal(result.outcome.status, 'unknown');
  assert.equal(attempt.pendingOperation, 'axes');
});

test('an arbitrary admission/auth rejection is not a durable axes decision', async () => {
  const attempt = new ProductWriteAttempt(
    memoryJournal(product, undefined, scope)
  );
  const result = await attempt.execute('axes', request(), async () => ({
    status: 'rejected',
    code: 'UNAUTHORIZED',
    message: 'not owner terminal proof'
  }));
  assert.equal(result.outcome.status, 'unknown');
  assert.equal(attempt.pendingOperation, 'axes');
});
