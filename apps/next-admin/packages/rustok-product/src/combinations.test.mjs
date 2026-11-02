/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import {
  combinationPage,
  chooseCombination,
  setCombinationFilter,
  CombinationError,
  retainedCombinationLabels
} from './combinations.ts';

const axis = (id, count) => ({
  attributeId: id,
  label: id,
  options: Array.from({ length: count }, (_, n) => ({
    id: `${id}-${n}`,
    label: String(n)
  }))
});
const value = (attributeId, optionId) => ({ attributeId, optionId });
const member = (id, axisValues) => ({ id, sku: `code-${id}`, axisValues });
const code = (expected) => (error) =>
  error instanceof CombinationError && error.code === expected;

test('mixed-radix pages are bounded, ordered and do not allocate the Cartesian matrix', () => {
  const axes = [axis('color', 4), axis('size', 4)];
  const first = combinationPage(axes, [], [], 0);
  const last = combinationPage(axes, [], [], 1);
  assert.deepEqual(
    [first.count, first.pages, first.rows.length, last.rows.length],
    [16, 2, 12, 4]
  );
  assert.deepEqual(last.rows[0].axisValues, [
    value('color', 'color-3'),
    value('size', 'size-0')
  ]);
  assert.equal(
    new Set([...first.rows, ...last.rows].map((row) => row.key)).size,
    16
  );
  const boundary = combinationPage(
    [axis('x', 64), axis('y', 64)],
    [],
    [],
    9999
  );
  assert.deepEqual(
    [boundary.count, boundary.page, boundary.rows.length],
    [4096, 341, 4]
  );
});

test('oversize refuses rather than returning a first-N matrix; narrowing is local only', () => {
  const axes = [axis('color', 256), axis('size', 256)];
  const before = structuredClone(axes);
  assert.throws(() => combinationPage(axes, [], [], 0), code('limit'));
  const filters = setCombinationFilter([], 'color', 'color-7');
  assert.equal(combinationPage(axes, filters, [], 0).count, 256);
  assert.deepEqual(axes, before);
  assert.deepEqual(setCombinationFilter(filters, 'color', ''), []);
  assert.deepEqual(filters, [value('color', 'color-7')]);
});

test('membership is full-tuple identity independent of axis order, labels or SKU code', () => {
  const axes = [axis('color', 2), axis('size', 2)];
  const values = [value('size', 'size-1'), value('color', 'color-0')];
  const source = [member('stable-uuid', values)];
  const first = combinationPage(axes, [], source, 0);
  const reordered = combinationPage([...axes].reverse(), [], source, 0);
  const occupied = first.rows.find((row) => row.existing);
  assert.equal(occupied.existing.id, 'stable-uuid');
  assert.equal(reordered.rows.find((row) => row.existing).key, occupied.key);
  assert.throws(
    () => chooseCombination(axes, [], source, [...values].reverse()),
    code('taken')
  );
  assert.equal(first.rows.filter((row) => row.existing === null).length, 3);
});

test('invalid, duplicate or oversized saved membership never becomes an available space', () => {
  const axes = [axis('color', 2)];
  const selected = [value('color', 'color-0')];
  for (const source of [
    [member('one', [])],
    [member('one', [value('color', 'removed')])],
    [member('one', selected), member('two', selected)],
    [
      member('same-id', selected),
      member('same-id', [value('color', 'color-1')])
    ],
    Array.from({ length: 257 }, (_, n) => member(String(n), selected))
  ])
    assert.throws(() => combinationPage(axes, [], source, 0), code('members'));
  // A filter cannot hide malformed/duplicate tuples elsewhere in the full snapshot.
  assert.throws(
    () =>
      combinationPage(
        axes,
        [value('color', 'color-1')],
        [member('one', selected), member('two', selected)],
        0
      ),
    code('members')
  );
});

test('incomplete and stale initial drafts remain present and explicitly unindexed', () => {
  const axes = [axis('color', 2)];
  const source = [
    member('incomplete', []),
    member('stale', [value('old-axis', 'old-value')])
  ];
  const before = structuredClone(source);
  const page = combinationPage(axes, [], source, 0, true);
  assert.equal(page.incompleteDrafts, 2);
  assert.equal(page.rows.filter((row) => row.existing === null).length, 2);
  assert.deepEqual(
    chooseCombination(axes, [], source, page.rows[0].axisValues, true),
    page.rows[0].axisValues
  );
  assert.deepEqual(source, before);
});

test('stale/duplicate filters, invalid pages and invalid axis dictionaries explicitly refuse', () => {
  const axes = [axis('color', 2)];
  for (const filters of [
    [value('old', 'removed')],
    [value('color', 'removed')],
    [value('color', 'color-0'), value('color', 'color-1')]
  ]) {
    const before = structuredClone(filters);
    assert.throws(() => combinationPage(axes, filters, [], 0), code('filters'));
    assert.deepEqual(filters, before);
  }
  for (const page of [-1, NaN, Infinity, 0.5])
    assert.throws(() => combinationPage(axes, [], [], page), code('filters'));
  for (const invalid of [
    [axis('x', 0)],
    [axis('x', 257)],
    [axis('x', 1), axis('x', 1)],
    Array.from({ length: 33 }, (_, n) => axis(String(n), 1)),
    [
      {
        ...axis('x', 2),
        options: [
          { id: 'same', label: 'a' },
          { id: 'same', label: 'b' }
        ]
      }
    ]
  ]) {
    assert.throws(() => combinationPage(invalid, [], [], 0), code('axes'));
  }
});

test('no axes means one empty default tuple, never permission for a second default SKU', () => {
  const page = combinationPage([], [], [], 0, true);
  assert.equal(page.count, 1);
  assert.deepEqual(page.rows[0].axisValues, []);
  assert.deepEqual(chooseCombination([], [], [], [], true), []);
  assert.throws(() => combinationPage([], [], [], 0), code('members'));
  assert.throws(
    () => chooseCombination([], [], [member('default-uuid', [])], []),
    code('taken')
  );
  assert.throws(
    () => combinationPage([], [], [member('one', []), member('two', [])], 0),
    code('members')
  );
});

test('selection rechecks current membership, filters and configuration and returns only assignments', () => {
  const axes = [axis('color', 2)];
  const candidate = combinationPage(axes, [], [], 0).rows[0];
  const occupiedNow = [member('created-after-preview', candidate.axisValues)];
  assert.throws(
    () => chooseCombination(axes, [], occupiedNow, candidate.axisValues),
    code('taken')
  );
  assert.throws(
    () =>
      chooseCombination(
        axes,
        [value('color', 'color-1')],
        [],
        candidate.axisValues
      ),
    code('filters')
  );
  assert.throws(
    () =>
      chooseCombination([axis('replacement', 2)], [], [], candidate.axisValues),
    code('filters')
  );
  const selected = chooseCombination(
    axes,
    [],
    [],
    candidate.axisValues.map((value) => ({
      ...value,
      price: '9.99',
      stock: 99,
      sku: 'never-copy'
    }))
  );
  assert.notEqual(selected, candidate.axisValues);
  assert.notEqual(selected[0], candidate.axisValues[0]);
  assert.deepEqual(Object.keys(selected[0]).sort(), [
    'attributeId',
    'optionId'
  ]);
  assert.equal(selected[0].optionId, 'color-0');
});

test('existing tuple presentation retains unknown assignments instead of displaying an empty/default tuple', () => {
  const axes = [
    {
      attributeId: 'color',
      name: 'Colour',
      allowedValues: [{ optionId: 'blue', value: 'Blue' }]
    }
  ];
  assert.deepEqual(retainedCombinationLabels(axes, [value('color', 'blue')]), [
    'Colour: Blue'
  ]);
  assert.deepEqual(
    retainedCombinationLabels(axes, [
      value('retained-attribute', 'retained-option')
    ]),
    ['retained-attribute: retained-option']
  );
});
