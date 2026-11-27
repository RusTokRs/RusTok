import test from 'node:test';
import assert from 'node:assert/strict';
import { requireSchemaWriteConfirmation } from './api/write-confirmation.ts';

test('schema writes require an explicit positive owner acknowledgement', () => {
  assert.equal(requireSchemaWriteConfirmation(true), true);
  for (const value of [false, null, undefined]) {
    assert.throws(
      () => requireSchemaWriteConfirmation(value),
      /Product schema write was not confirmed by the owner/
    );
  }
});
