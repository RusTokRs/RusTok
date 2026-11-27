import test from 'node:test';
import assert from 'node:assert/strict';
import {
  requireOwnerResult,
  requireOwnerWriteConfirmation,
  requireSchemaWriteConfirmation
} from './api/write-confirmation.ts';

test('schema writes require an explicit positive owner acknowledgement', () => {
  assert.equal(requireSchemaWriteConfirmation(true), true);
  for (const value of [false, null, undefined]) {
    assert.throws(
      () => requireSchemaWriteConfirmation(value),
      /Product schema write was not confirmed by the owner/
    );
  }
});

test('resource mutations reject missing results and false acknowledgements', () => {
  assert.deepEqual(requireOwnerResult({ id: 'created' }, 'Resource creation'), {
    id: 'created'
  });
  assert.throws(
    () => requireOwnerResult(null, 'Resource creation'),
    /Resource creation did not return a result from the owner/
  );
  assert.equal(requireOwnerWriteConfirmation(true, 'Resource deletion'), true);
  assert.throws(
    () => requireOwnerWriteConfirmation(false, 'Resource deletion'),
    /Resource deletion was not confirmed by the owner/
  );
});
