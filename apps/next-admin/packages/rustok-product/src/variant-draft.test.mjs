/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import {
  axisPolicyCore,
  variantDraftCore
} from './variant-draft-test-support.mjs';
const {
  normalizeInitialCatalog,
  suggestedInitialAxes,
  emptyInitialVariant,
  initialVariantInput
} = variantDraftCore;
const clone = (value) => JSON.parse(JSON.stringify(value));
const attribute = (id, patch = {}) => ({
  attributeId: id,
  code: id,
  label: id,
  scope: 'variant',
  valueType: 'select',
  variantAxisPolicy: 'allowed',
  defaultVariantAxis: true,
  isDisabled: false,
  source: 'schema',
  options: [{ id: `${id}-a` }, { id: `${id}-b` }],
  ...patch
});
const form = {
  categoryId: 'category',
  attributes: [attribute('color'), attribute('size')],
  detachedAttributeIds: []
};
const axes = suggestedInitialAxes(form);
const row = (color, size, sku = `${color}-${size}`) => ({
  ...emptyInitialVariant(),
  sku,
  priceAmount: '12.500001',
  inventoryQuantity: 3,
  axisValues: [
    { attributeId: 'color', optionId: `color-${color}` },
    { attributeId: 'size', optionId: `size-${size}` }
  ]
});

test('policy omission is a patch, not a reset, and unknown values fail closed', () => {
  assert.deepEqual(
    clone(axisPolicyCore.buildAxisPolicyPatch('keep', 'keep')),
    {}
  );
  assert.deepEqual(
    clone(axisPolicyCore.buildAxisPolicyPatch('required', 'false')),
    { variantAxisPolicy: 'required', defaultVariantAxis: false }
  );
  assert.throws(
    () => axisPolicyCore.buildAxisPolicyPatch('forbidden', 'true'),
    /forbidden/
  );
  assert.throws(
    () => axisPolicyCore.buildAxisPolicyPatch('future-policy', 'keep'),
    /Invalid/
  );
});

test('only active select attributes with variant/both scope and an allowed policy are offered', () => {
  for (const patch of [
    { scope: 'product' },
    { valueType: 'multiselect' },
    { isDisabled: true },
    { variantAxisPolicy: 'forbidden' },
    { variantAxisPolicy: 'unknown' }
  ]) {
    assert.equal(axisPolicyCore.isEligibleAxis(attribute('a', patch)), false);
  }
  assert.equal(
    axisPolicyCore.isEligibleAxis(attribute('a', { scope: 'both' })),
    true
  );
});

test('two selected tuples in a four-combination space stay sparse with exact prices and stock', () => {
  const variants = normalizeInitialCatalog(
    axes,
    [row('a', 'a'), row('b', 'b')],
    form
  );
  assert.equal(variants.length, 2);
  assert.equal(variants[0].priceAmount, '12.500001');
  assert.equal(initialVariantInput(variants[1]).inventoryQuantity, 3);
  assert.equal(
    initialVariantInput(variants[1]).axisValues[0].optionId,
    'color-b'
  );
});

test('missing/duplicate assignments and duplicate tuples are refused before the journal write', () => {
  assert.throws(
    () =>
      normalizeInitialCatalog(
        axes,
        [
          {
            ...row('a', 'a'),
            axisValues: [{ attributeId: 'color', optionId: 'color-a' }]
          }
        ],
        form
      ),
    /exactly one/
  );
  assert.throws(
    () =>
      normalizeInitialCatalog(
        axes,
        [
          {
            ...row('a', 'a'),
            axisValues: [
              { attributeId: 'color', optionId: 'color-a' },
              { attributeId: 'color', optionId: 'color-b' }
            ]
          }
        ],
        form
      ),
    /duplicate axis/
  );
  assert.throws(
    () =>
      normalizeInitialCatalog(
        axes,
        [row('a', 'a'), { ...row('a', 'a'), sku: 'other' }],
        form
      ),
    /same axis combination/
  );
});

test('removed values and category changes retain the original draft but prevent invalid writes', () => {
  const draft = [row('a', 'a')];
  const original = clone(draft);
  assert.throws(
    () =>
      normalizeInitialCatalog(
        axes.map((axis) =>
          axis.attributeId === 'color'
            ? { ...axis, allowedOptionIds: ['color-b'] }
            : axis
        ),
        draft,
        form
      ),
    /outside/
  );
  assert.throws(
    () => normalizeInitialCatalog(axes, draft, { ...form, attributes: [] }),
    /not allowed/
  );
  assert.deepEqual(clone(draft), original);
});

test('required axes do not depend on the optional default hint', () => {
  const required = {
    ...form,
    attributes: [
      attribute('color', {
        defaultVariantAxis: false,
        variantAxisPolicy: 'required'
      })
    ]
  };
  assert.equal(suggestedInitialAxes(required).length, 1);
  assert.throws(
    () => normalizeInitialCatalog([], [emptyInitialVariant()], required),
    /Required axis/
  );
});

test('a simple product has one default SKU and no assignments', () => {
  assert.equal(
    normalizeInitialCatalog([], [emptyInitialVariant()], null).length,
    1
  );
  assert.throws(() => normalizeInitialCatalog([], [], null), /default SKU/);
  assert.throws(
    () =>
      normalizeInitialCatalog(
        [],
        [emptyInitialVariant(), emptyInitialVariant()],
        null
      ),
    /default SKU/
  );
});

test('variant/axis/value bounds and invalid scalar drafts are rejected, never truncated', () => {
  assert.throws(
    () =>
      normalizeInitialCatalog(
        axes,
        Array.from({ length: 129 }, () => row('a', 'a')),
        form
      ),
    /1–128/
  );
  assert.throws(
    () =>
      suggestedInitialAxes({
        ...form,
        attributes: [
          attribute('large', {
            options: Array.from({ length: 257 }, (_, id) => ({ id: `${id}` }))
          })
        ]
      }),
    /1–256/
  );
  for (const patch of [
    { priceAmount: '-1' },
    { inventoryQuantity: NaN },
    { inventoryQuantity: 0.5 },
    { inventoryQuantity: 2147483648 }
  ]) {
    assert.throws(() =>
      normalizeInitialCatalog(
        [],
        [{ ...emptyInitialVariant(), ...patch }],
        null
      )
    );
  }
});

test('axis edits renumber the retained identities without pruning selected SKUs', () => {
  const retained = variantDraftCore.setInitialAxis(axes, 'color', form, false);
  assert.equal(retained[0].attributeId, 'size');
  assert.equal(retained[0].position, 0);
  const added = variantDraftCore.setInitialAxis(retained, 'color', form, true);
  assert.deepEqual(clone(added.map((axis) => axis.position)), [0, 1]);
  const narrowed = variantDraftCore.setInitialAllowedValue(
    added,
    'color',
    'color-a',
    false
  );
  const draft = row('a', 'a');
  assert.throws(
    () => normalizeInitialCatalog(narrowed, [draft], form),
    /outside/
  );
  assert.equal(draft.axisValues[0].optionId, 'color-a');
  const corrected = {
    ...draft,
    axisValues: variantDraftCore.setInitialAssignment(draft, 'color', 'color-b')
  };
  assert.equal(normalizeInitialCatalog(narrowed, [corrected], form).length, 1);
});

test('explicit combination selection adds one local draft with no inferred price, SKU or stock', () => {
  const before = [row('a', 'a')];
  const next = variantDraftCore.appendInitialCombination(
    axes,
    before,
    form,
    row('b', 'b').axisValues
  );
  assert.equal(before.length, 1);
  assert.equal(next.length, 2);
  assert.equal(next[0], before[0]);
  assert.equal(next[1].sku, '');
  assert.equal(next[1].barcode, '');
  assert.equal(next[1].priceAmount, '');
  assert.equal(next[1].inventoryQuantity, 0);
  assert.equal(initialVariantInput(next[1]).prices.length, 0);
  assert.throws(
    () =>
      variantDraftCore.appendInitialCombination(
        axes,
        next,
        form,
        row('b', 'b').axisValues
      ),
    (error) => error.code === 'taken'
  );
  assert.equal(next.length, 2);
  assert.throws(
    () =>
      variantDraftCore.appendInitialCombination(
        axes,
        Array.from({ length: 128 }, () => row('a', 'a')),
        form,
        row('b', 'b').axisValues
      ),
    (error) => error.code === 'capacity'
  );
});

test('blank initial price is absent, explicit zero remains a price, orphan compare-at refuses', () => {
  const blank = normalizeInitialCatalog([], [emptyInitialVariant()], null)[0];
  assert.equal(initialVariantInput(blank).prices.length, 0);
  const zero = normalizeInitialCatalog(
    [],
    [
      {
        ...emptyInitialVariant(),
        priceAmount: '0',
        compareAtAmount: '1.000001'
      }
    ],
    null
  )[0];
  assert.equal(initialVariantInput(zero).prices[0].amount, '0');
  assert.equal(initialVariantInput(zero).prices[0].compareAtAmount, '1.000001');
  const orphan = { ...emptyInitialVariant(), compareAtAmount: '1' };
  assert.throws(
    () => normalizeInitialCatalog([], [orphan], null),
    /explicit base price/
  );
  assert.throws(() => initialVariantInput(orphan), /explicit base price/);
});

// The option ontology and required-axis rules are the same ones used by ordinary save.
test('candidate projection uses the configured allowed subset and refuses stale/required axes', () => {
  const subset = [{ attributeId: 'color', allowedOptionIds: ['color-b'] }];
  const projected = variantDraftCore.initialCombinationAxes(subset, form);
  assert.equal(projected.length, 1);
  assert.equal(projected[0].attributeId, 'color');
  assert.deepEqual(clone(projected[0].options.map((option) => option.id)), [
    'color-b'
  ]);
  assert.throws(
    () =>
      variantDraftCore.initialCombinationAxes(
        [{ attributeId: 'removed', allowedOptionIds: ['old'] }],
        form
      ),
    (error) => error.code === 'axes'
  );
  assert.throws(
    () =>
      variantDraftCore.initialCombinationAxes([], {
        ...form,
        attributes: [attribute('required', { variantAxisPolicy: 'required' })]
      }),
    (error) => error.code === 'axes'
  );
});

test('a picked full tuple can be appended when the unfiltered Cartesian space exceeds the browser bound', () => {
  const attributes = ['color', 'size'].map((id) =>
    attribute(id, {
      options: Array.from({ length: 256 }, (_, n) => ({
        id: `${id}-${n}`,
        label: String(n)
      }))
    })
  );
  const largeForm = { ...form, attributes };
  const configured = attributes.map((attribute) => ({
    attributeId: attribute.attributeId,
    allowedOptionIds: attribute.options.map((option) => option.id)
  }));
  const chosen = [
    { attributeId: 'color', optionId: 'color-7' },
    { attributeId: 'size', optionId: 'size-20' }
  ];
  const rows = variantDraftCore.appendInitialCombination(
    configured,
    [],
    largeForm,
    chosen
  );
  assert.equal(rows.length, 1);
  assert.deepEqual(clone(rows[0].axisValues), chosen);
  assert.equal(initialVariantInput(rows[0]).prices.length, 0);
  assert.equal(configured[0].allowedOptionIds.length, 256);
});
