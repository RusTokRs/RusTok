import assert from 'node:assert/strict';
import test from 'node:test';

import {
  MAX_PROTOCOL_OVERHEAD_BYTES,
  canonicalRichTextLocale,
  createEnvelope,
  createRichTextAuthoringContext,
  emptyRichTextDocument,
  getRichTextProfile,
  isEnvelope,
  isRichTextAuthoringContext,
  richTextDocumentHasText,
  validateRichTextDocument
} from '../dist/core.mjs';

test('authoring context is canonical, locale-neutral, and bounded', () => {
  const arabic = createRichTextAuthoringContext({
    contentLocale: ' ar ',
    direction: 'rtl'
  });
  assert.deepEqual(arabic, {
    locale: 'ar',
    direction: 'rtl',
    spellcheck: true
  });
  assert.equal(isRichTextAuthoringContext(arabic), true);
  assert.equal(canonicalRichTextLocale('not a locale'), 'und');
  assert.deepEqual(
    createRichTextAuthoringContext({ contentLocale: '' }),
    { locale: 'und', direction: 'auto', spellcheck: true }
  );
  assert.equal(
    isRichTextAuthoringContext({ ...arabic, locale: ' ar ' }),
    false
  );
});

test('empty document uses an editor-compatible paragraph', () => {
  const document = emptyRichTextDocument();
  assert.deepEqual(document, {
    type: 'doc',
    content: [{ type: 'paragraph' }]
  });
  const article = getRichTextProfile('article');
  assert.equal(validateRichTextDocument(document, article).valid, false);
  assert.equal(
    validateRichTextDocument(document, article, { allowEmpty: true }).valid,
    true
  );
});

test('text presence follows structural text nodes', () => {
  assert.equal(richTextDocumentHasText(emptyRichTextDocument()), false);
  assert.equal(
    richTextDocumentHasText({
      type: 'doc',
      content: [{ type: 'paragraph', content: [{ type: 'text', text: '  body  ' }] }]
    }),
    true
  );
});

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

test('browser validation enforces the Rust tree grammar', () => {
  const article = getRichTextProfile('article');
  const invalidDocuments = [
    { type: 'doc', content: [{ type: 'text', text: 'root inline' }] },
    {
      type: 'doc',
      content: [{ type: 'paragraph', content: [{ type: 'heading', attrs: { level: 2 }, content: [] }] }]
    },
    {
      type: 'doc',
      content: [{ type: 'bulletList', content: [{ type: 'paragraph', content: [{ type: 'text', text: 'item' }] }] }]
    },
    {
      type: 'doc',
      content: [{ type: 'codeBlock', content: [{ type: 'text', text: 'code', marks: [{ type: 'bold' }] }] }]
    },
    {
      type: 'doc',
      content: [{ type: 'paragraph', marks: [{ type: 'bold' }], content: [{ type: 'text', text: 'body' }] }]
    }
  ];

  for (const document of invalidDocuments) {
    assert.equal(validateRichTextDocument(document, article).valid, false);
  }
});

test('browser validation rejects duplicate marks and oversized ordered-list starts', () => {
  const article = getRichTextProfile('article');
  assert.equal(
    validateRichTextDocument(
      {
        type: 'doc',
        content: [{
          type: 'paragraph',
          content: [{ type: 'text', text: 'body', marks: [{ type: 'bold' }, { type: 'bold' }] }]
        }]
      },
      article
    ).valid,
    false
  );
  assert.equal(
    validateRichTextDocument(
      {
        type: 'doc',
        content: [{
          type: 'orderedList',
          attrs: { start: 1_000_001 },
          content: [{ type: 'listItem', content: [{ type: 'paragraph', content: [{ type: 'text', text: 'body' }] }] }]
        }]
      },
      article
    ).valid,
    false
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

test('client link policy matches the Rust renderer boundary', () => {
  const discussion = getRichTextProfile('discussion');
  const documentWithHref = (href) => ({
    type: 'doc',
    content: [
      {
        type: 'paragraph',
        content: [
          {
            type: 'text',
            text: 'link',
            marks: [{ type: 'link', attrs: { href } }]
          }
        ]
      }
    ]
  });

  for (const href of [
    '/posts/hello',
    '#section',
    'https://example.com/path',
    'http://example.com',
    'mailto:editor@example.com'
  ]) {
    assert.equal(
      validateRichTextDocument(documentWithHref(href), discussion).valid,
      true,
      `${href} should be accepted`
    );
  }

  for (const href of [
    '',
    '#',
    '//example.com',
    '/\\example.com',
    'https://user:secret@example.com',
    ' https://example.com',
    'javascript:alert(1)',
    'https://example.com/\nheader'
  ]) {
    assert.equal(
      validateRichTextDocument(documentWithHref(href), discussion).valid,
      false,
      `${JSON.stringify(href)} should be rejected`
    );
  }
});

test('protocol rejects replayed and oversized envelopes', () => {
  const session = '00000000-0000-4000-8000-000000000000';
  const envelope = createEnvelope(session, 2, { type: 'focus', payload: {} });
  assert.equal(Object.hasOwn(envelope, 'revision'), false);
  assert.equal(isEnvelope(envelope, session, 1, MAX_PROTOCOL_OVERHEAD_BYTES), true);
  assert.equal(isEnvelope(envelope, session, 2, MAX_PROTOCOL_OVERHEAD_BYTES), false);
  assert.equal(isEnvelope(envelope, session, 1, 8), false);
});
