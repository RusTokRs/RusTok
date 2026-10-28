import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { compileFunction } from 'node:vm';
import test from 'node:test';
import {
  emptyRichTextDocument,
  richTextDocumentHasText,
  validateRichTextDocument
} from '../../../../../../packages/richtext/src/document.ts';

const source = readFileSync(
  new URL('./forum-reply-editor.tsx', import.meta.url),
  'utf8'
);
const profiles = JSON.parse(
  readFileSync(
    new URL(
      '../../../../../../packages/richtext/src/generated/profiles.json',
      import.meta.url
    ),
    'utf8'
  )
);
const profile = profiles.find((item) => item.id === 'discussion');
const submitSource = stripTypeScriptTypes(
  source.slice(
    source.indexOf('  async function submit('),
    source.indexOf('\n\n  return (', source.indexOf('  async function submit('))
  )
);

const document = (text) => ({
  type: 'doc',
  content: [{ type: 'paragraph', content: [{ type: 'text', text }] }]
});
function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

function harness(transport, initial = document('Reply body')) {
  const resets = [];
  const notices = [];
  const bindings = {
    topicId: 'topic-a',
    gqlOpts: { tenantId: 'tenant', token: 'token' },
    documentRef: { current: initial },
    submittingRef: { current: false },
    mountedRef: { current: true },
    validateRichTextDocument,
    richTextDocumentHasText,
    emptyRichTextDocument,
    getRichTextProfile: (id) => {
      assert.equal(id, 'discussion');
      return profile;
    },
    createForumReply: transport,
    form: { reset: (value) => resets.push(value) },
    toast: {
      success: (message) => notices.push(['success', message]),
      error: (message) => notices.push(['error', message])
    }
  };
  bindings.updateDocument = (value) => {
    bindings.documentRef.current = value;
  };
  const submit = compileFunction(
    `${submitSource}\nreturn submit;`,
    Object.keys(bindings)
  )(...Object.values(bindings));
  return { submit, bindings, resets, notices };
}

test('successful reply clears its document, keeps its locale, and cannot be submitted twice', async () => {
  let calls = 0;
  const h = harness(async (topicId, input, opts) => {
    calls++;
    assert.equal(topicId, 'topic-a');
    assert.equal(input.locale, 'ru');
    assert.equal(input.content, h.bindings.documentRef.current);
    assert.deepEqual(opts, { tenantId: 'tenant', token: 'token' });
    return 'reply-id';
  });
  await h.submit({ locale: 'ru' });
  assert.deepEqual(h.bindings.documentRef.current, emptyRichTextDocument());
  assert.deepEqual(h.resets, [{ locale: 'ru' }]);
  assert.deepEqual(h.notices, [['success', 'Reply posted']]);
  // Even a same-render stale handler reads the newest document ref.
  await h.submit({ locale: 'ru' });
  assert.equal(calls, 1);
  assert.equal(h.bindings.submittingRef.current, false);
});

test('failed reply preserves content and locale and releases the retry lock', async () => {
  let calls = 0;
  const initial = document('Keep my reply');
  const h = harness(async () => {
    if (++calls === 1) throw new Error('offline');
    return 'id';
  }, initial);
  await h.submit({ locale: 'ar' });
  assert.equal(h.bindings.documentRef.current, initial);
  assert.deepEqual(h.resets, []);
  assert.deepEqual(h.notices, [['error', 'Failed to post reply']]);
  assert.equal(h.bindings.submittingRef.current, false);
  await h.submit({ locale: 'ar' });
  assert.equal(calls, 2);
  assert.deepEqual(h.resets, [{ locale: 'ar' }]);
});

test('a second pending submit does not create another reply', async () => {
  const gate = deferred();
  let calls = 0;
  const h = harness(() => {
    calls++;
    return gate.promise;
  });
  const pending = h.submit({ locale: 'en' });
  await h.submit({ locale: 'en' });
  assert.equal(calls, 1);
  gate.resolve('id');
  await pending;
});

test('success cannot erase genuinely newer frame input', async () => {
  const gate = deferred();
  const h = harness(() => gate.promise);
  const pending = h.submit({ locale: 'en' });
  const newer = document('New input received during the request');
  h.bindings.updateDocument(newer);
  gate.resolve('id');
  await pending;
  assert.equal(h.bindings.documentRef.current, newer);
  assert.deepEqual(h.resets, []);
});

test('an equal document re-emitted by the read-only frame is still cleared after success', async () => {
  const gate = deferred();
  const h = harness(() => gate.promise);
  const pending = h.submit({ locale: 'en' });
  h.bindings.updateDocument(structuredClone(h.bindings.documentRef.current));
  gate.resolve('id');
  await pending;
  assert.deepEqual(h.bindings.documentRef.current, emptyRichTextDocument());
  assert.deepEqual(h.resets, [{ locale: 'en' }]);
});

test('old session completions do not clear a new topic editor or emit stale notices', async () => {
  for (const fails of [false, true]) {
    const gate = deferred();
    const initial = document('Old session draft');
    const h = harness(() => gate.promise, initial);
    const pending = h.submit({ locale: 'en' });
    h.bindings.mountedRef.current = false;
    if (fails) gate.reject(new Error('late error'));
    else gate.resolve('id');
    await pending;
    assert.equal(h.bindings.documentRef.current, initial);
    assert.deepEqual(h.resets, []);
    assert.deepEqual(h.notices, []);
  }
  assert.match(source, /<ForumReplyEditorSession key=\{sessionKey\}/);
  assert.match(source, /props\.topicId/);
  assert.match(source, /props\.gqlOpts\?\.tenantId/);
  assert.match(source, /mountedRef\.current = false/);
});

test('empty or invalid discussion documents never reach the owner write', async () => {
  for (const value of [
    emptyRichTextDocument(),
    { type: 'doc', content: [{ type: 'unsafe', text: 'bad' }] }
  ]) {
    const h = harness(() => assert.fail('unexpected request'), value);
    await h.submit({ locale: 'en' });
    assert.equal(h.bindings.documentRef.current, value);
    assert.equal(h.bindings.submittingRef.current, false);
    assert.deepEqual(h.resets, []);
  }
});
