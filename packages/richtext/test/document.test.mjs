import assert from 'node:assert/strict';
import test from 'node:test';

import {
  MAX_PROTOCOL_OVERHEAD_BYTES,
  createEnvelope,
  getRichTextProfile,
  isEnvelope,
  validateRichTextDocument
} from '../dist/core.mjs';

test('profile manifests keep comment authoring intentionally small', () => {
  const comment = getRichTextProfile('comment');
  assert.equal(comment.nodes.includes('heading'), false);
  assert.equal(comment.nodes.includes('codeBlock'), false);
  assert.deepEqual(comment.heading_levels, []);
});

test('document validation follows the server-selected heading profile', () => {
  const article = getRichTextProfile('article');
  assert.equal(
    validateRichTextDocument(
      {
        type: 'doc',
        content: [{ type: 'heading', attrs: { level: 2 }, content: [{ type: 'text', text: 'Title' }] }]
      },
      article
    ).valid,
    true
  );
  assert.match(
    validateRichTextDocument(
      { type: 'doc', content: [{ type: 'heading', attrs: { level: 1 }, content: [] }] },
      article
    ).error ?? '',
    /invalid heading attributes/
  );
});

test('client guard rejects link presentation policy and unsafe schemes', () => {
  const discussion = getRichTextProfile('discussion');
  for (const attrs of [{ href: 'javascript:alert(1)' }, { href: 'https://example.com', rel: 'opener' }]) {
    const result = validateRichTextDocument(
      {
        type: 'doc',
        content: [{ type: 'paragraph', content: [{ type: 'text', text: 'link', marks: [{ type: 'link', attrs }] }] }]
      },
      discussion
    );
    assert.equal(result.valid, false);
  }
});

test('protocol rejects replayed and oversized envelopes', () => {
  const session = '00000000-0000-4000-8000-000000000000';
  const envelope = createEnvelope(session, 2, { type: 'focus', payload: {} });
  assert.equal(isEnvelope(envelope, session, 1, MAX_PROTOCOL_OVERHEAD_BYTES), true);
  assert.equal(isEnvelope(envelope, session, 2, MAX_PROTOCOL_OVERHEAD_BYTES), false);
  assert.equal(isEnvelope(envelope, session, 1, 8), false);
});
