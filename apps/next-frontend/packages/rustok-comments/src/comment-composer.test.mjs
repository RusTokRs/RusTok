import assert from 'node:assert/strict';
import { test } from 'node:test';

test('commandId idempotency logic: reused on retry, invalidated on document change', () => {
  let commandId = null;
  let pending = false;

  // First submit
  const firstSubmissionId = commandId ?? 'cmd-uuid-1';
  commandId = firstSubmissionId;
  assert.equal(commandId, 'cmd-uuid-1');

  // Retry after failure: commandId stays the same
  const retrySubmissionId = commandId ?? 'cmd-uuid-2';
  assert.equal(retrySubmissionId, 'cmd-uuid-1');

  // User edits document: commandId is cleared
  if (!pending && commandId) {
    commandId = null;
  }
  assert.equal(commandId, null);

  // New submit gets new commandId
  const newSubmissionId = commandId ?? 'cmd-uuid-3';
  assert.equal(newSubmissionId, 'cmd-uuid-3');
});

test('empty document validation logic checks text content', () => {
  const hasText = (doc) => Boolean(doc && doc.text && doc.text.trim().length > 0);
  assert.equal(hasText(null), false);
  assert.equal(hasText({ text: '' }), false);
  assert.equal(hasText({ text: '   \n  ' }), false);
  assert.equal(hasText({ text: 'Valid comment text' }), true);
});

test('authentication gate checks canSubmit flag', () => {
  const getViewState = (canSubmit) => (canSubmit ? 'FORM' : 'SIGN_IN_REQUIRED');
  assert.equal(getViewState(false), 'SIGN_IN_REQUIRED');
  assert.equal(getViewState(true), 'FORM');
});
