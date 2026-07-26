#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import assert from 'node:assert/strict';
import { test } from 'node:test';

const schema = JSON.parse(readFileSync(
  new URL('../../crates/rustok-index/docs/storage-decision.schema.json', import.meta.url),
  'utf8',
));

const prototypes = ['jsonb', 'typed_eav', 'hot_projection'];
const requiredTopLevelText = [
  'owner',
  'selection_rationale',
  'operational_tradeoffs',
  'migration_strategy',
  'rollback_strategy',
];

const requireTextContract = (definition, label) => {
  assert.equal(definition?.type, 'string', `${label} must be a string`);
  assert.equal(definition?.minLength, 1, `${label} must retain minLength 1`);
  assert.equal(definition?.pattern, '\\S', `${label} must require one non-whitespace character`);
};

const selectedBranch = (prototype) => schema.allOf.find(
  (entry) => entry?.if?.properties?.selected_prototype?.const === prototype,
);

const acceptsText = (definition, value) => typeof value === 'string'
  && value.length >= definition.minLength
  && new RegExp(definition.pattern, 'u').test(value);

const everyTextDefinition = () => {
  const definitions = requiredTopLevelText.map((field) => [
    field,
    schema.properties[field],
  ]);
  for (const prototype of prototypes) {
    const branch = selectedBranch(prototype);
    for (const [rejected, definition] of Object.entries(
      branch?.then?.properties?.rejection_rationales?.properties ?? {},
    )) {
      definitions.push([`${prototype}.rejection_rationales.${rejected}`, definition]);
    }
  }
  return definitions;
};

test('requires non-whitespace text for every top-level decision narrative', () => {
  for (const field of requiredTopLevelText) {
    requireTextContract(schema.properties[field], field);
  }
});

test('requires non-whitespace text for every conditional rejection rationale', () => {
  for (const selected of prototypes) {
    const branch = selectedBranch(selected);
    assert.ok(branch, `missing schema branch for ${selected}`);
    const rejection = branch.then.properties.rejection_rationales;
    const expected = prototypes.filter((prototype) => prototype !== selected);
    assert.deepEqual(rejection.required, expected);
    assert.deepEqual(Object.keys(rejection.properties), expected);
    assert.equal(rejection.additionalProperties, false);
    for (const rejected of expected) {
      requireTextContract(
        rejection.properties[rejected],
        `${selected}.rejection_rationales.${rejected}`,
      );
    }
  }
});

test('rejects empty and whitespace-only values while accepting reviewed text', () => {
  for (const [label, definition] of everyTextDefinition()) {
    for (const value of ['', ' ', '   ', '\n', '\t', ' \n\t ']) {
      assert.equal(acceptsText(definition, value), false, `${label} accepted ${JSON.stringify(value)}`);
    }
    for (const value of ['reviewed', ' reviewed ', '\nreviewed\t']) {
      assert.equal(acceptsText(definition, value), true, `${label} rejected ${JSON.stringify(value)}`);
    }
  }
});
