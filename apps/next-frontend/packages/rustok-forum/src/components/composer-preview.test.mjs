import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const composer = readFileSync(
  new URL('./composer.tsx', import.meta.url),
  'utf8',
);

test('preview passes the complete controlled document to the canonical discussion frame', () => {
  assert.equal(composer.match(/<RichTextEditor\s/g)?.length, 1);
  assert.match(
    composer,
    /import \{ RichTextEditor \} from '@rustok\/richtext\/react'/,
  );
  assert.match(
    composer,
    /<RichTextEditor[\s\S]*?frameUrl="\/richtext\/frame"[\s\S]*?profile="discussion"[\s\S]*?value=\{state\.document\}/,
  );
  assert.doesNotMatch(composer, /state\.document\.content\.map/);
  assert.doesNotMatch(composer, /dangerouslySetInnerHTML/);
});

test('preview is the existing frame made read-only, not a second lossy renderer or duplicate quotes', () => {
  assert.match(
    composer,
    /disabled=\{state\.isSubmitting \|\| activeTab === 'preview'\}/,
  );
  assert.match(composer, /contentLocale=\{locale\}/);
  assert.match(composer, /messages=\{richTextMessages\}/);
  assert.doesNotMatch(composer, /<blockquote/);
  assert.match(composer, /forum\('livePreview'\)/);
  assert.match(
    composer,
    /label=\{activeTab === 'preview' \? forum\('preview'\)/,
  );
});

async function submitHarness(mode, transport) {
  const { stripTypeScriptTypes } = await import('node:module');
  const { compileFunction } = await import('node:vm');
  const start = composer.indexOf('  async function handleSubmit(');
  const end = composer.indexOf('\n\n  return (', start);
  const posts = [],
    resets = [],
    errors = [];
  const bindings = {
    sessionCurrent: true,
    isEditingSession: () => bindings.sessionCurrent,
    state: {
      mode,
      isSubmitting: false,
      title: 'Title',
      document: { type: 'doc', content: [] },
      topicId: 'topic',
      parentReplyId: null,
      quotes: [],
    },
    submittingRef: { current: false },
    richTextDocumentHasText: () => true,
    isCurrentViewer: () => true,
    getClientAuth: () => ({ token: 'signed', tenant: 'shop' }),
    tenantId: 'tenant',
    tenantSlug: 'shop',
    locale: 'en',
    selectedCategoryId: 'category',
    setSubmitting: () => {},
    setError: (value) => errors.push(value),
    forum: (id) => id,
    reset: () => {
      resets.push('reset');
      bindings.sessionCurrent = false;
    },
    onSubmitted: () => {},
    createForumReply: (options) => {
      posts.push(options);
      return transport();
    },
    createForumTopic: (options) => {
      posts.push(options);
      return transport();
    },
  };
  const submit = compileFunction(
    `${stripTypeScriptTypes(composer.slice(start, end))}\nreturn handleSubmit;`,
    Object.keys(bindings),
  )(...Object.values(bindings));
  return {
    submit: () => submit({ preventDefault() {} }),
    bindings,
    posts,
    resets,
    errors,
  };
}

for (const mode of ['reply', 'topic']) {
  test(`${mode}: concurrent submits before the next render create only one write`, async () => {
    let resolve;
    const gate = new Promise((yes) => {
      resolve = yes;
    });
    const h = await submitHarness(mode, () => gate);
    const first = h.submit(),
      second = h.submit();
    resolve({ id: 'created' });
    await Promise.all([first, second]);
    assert.equal(h.posts.length, 1);
    assert.equal(h.resets.length, 1);
    assert.equal(h.bindings.submittingRef.current, false);
  });
}

test('a failed write releases the synchronous lock and retains the editor for retry', async () => {
  let attempts = 0;
  const h = await submitHarness('reply', async () => {
    if (++attempts === 1) throw new Error('offline');
    return { id: 'created' };
  });
  await h.submit();
  assert.equal(h.bindings.submittingRef.current, false);
  assert.equal(h.resets.length, 0);
  assert.equal(h.errors.at(-1), 'offline');
  await h.submit();
  assert.equal(h.posts.length, 2);
  assert.equal(h.resets.length, 1);
});

test('pending writes freeze title and quote changes, not only the richtext frame/category', () => {
  const titleInput = composer
    .match(/<input[\s\S]*?\/>/g)
    ?.find((tag) => tag.includes('value={state.title}'));
  assert.ok(titleInput);
  assert.match(titleInput, /disabled=\{state.isSubmitting\}/);
  assert.match(
    composer,
    /onClick=\{\(\) => removeQuote\(idx\)\}\s*disabled=\{state.isSubmitting\}/,
  );
});

async function documentChangeHarness() {
  const { stripTypeScriptTypes } = await import('node:module');
  const { compileFunction } = await import('node:vm');
  const frame = composer.match(/<RichTextEditor\s[\s\S]*?\/>/)[0];
  const callbackName = frame.match(/onChange=\{(\w+)\}/)[1];
  const documents = [];
  let generation = 0;
  const bindings = {
    setDocument: (document) => documents.push(document),
    captureEditorSession: () => {
      const captured = generation;
      return () => captured === generation;
    },
  };
  let change = bindings[callbackName];
  if (!change) {
    const start = composer.indexOf(
      '  const isEditingSession = captureEditorSession();',
    );
    const end = composer.indexOf('  function handleCategoryChange', start);
    assert.ok(start >= 0 && end > start);
    change = compileFunction(
      `${stripTypeScriptTypes(composer.slice(start, end))}\nreturn ${callbackName};`,
      Object.keys(bindings),
    )(...Object.values(bindings));
  }
  return {
    change,
    documents,
    replace: () => {
      generation++;
    },
  };
}

test('a delayed frame change cannot overwrite the document after a target/session replacement', async () => {
  const h = await documentChangeHarness();
  h.replace();
  h.change({
    type: 'doc',
    content: [
      { type: 'paragraph', content: [{ type: 'text', text: 'old editor' }] },
    ],
  });
  assert.deepEqual(h.documents, []);
});

test('each editor lifetime owns a distinct richtext frame, not just a new controlled value', () => {
  const frame = composer.match(/<RichTextEditor\s[\s\S]*?\/>/)[0];
  assert.match(frame, /key=\{state.editorSession\}/);
});

test('ordinary changes from the current frame still update its controlled document', async () => {
  const h = await documentChangeHarness();
  const document = { type: 'doc', content: [{ type: 'paragraph' }] };
  h.change(document);
  assert.deepEqual(h.documents, [document]);
});

for (const mode of ['reply', 'topic']) {
  test(`${mode}: a submit callback captured by a replaced/closed editor cannot create a write`, async () => {
    const h = await submitHarness(mode, async () => ({ id: 'created' }));
    h.bindings.sessionCurrent = false;
    await h.submit();
    assert.deepEqual(
      h.posts,
      [],
      'the old render must not post its captured draft/target',
    );
    assert.deepEqual(h.resets, []);
    assert.deepEqual(h.errors, []);
    assert.equal(h.bindings.submittingRef.current, false);
  });
}

test('composer headings and quote label are catalog-owned, including the reply topic fallback', () => {
  assert.doesNotMatch(composer, /Replying to:|Create New Topic|> Quotes:/);
  assert.match(
    composer,
    /forum\('replyingTo', \{ title: state.topicTitle \?\? forum\('topic'\) \}\)/,
  );
  assert.match(composer, /forum\('quotes'\)/);
});

for (const mode of ['reply', 'topic']) {
  test(`${mode}: successful cleanup invalidates the captured form before another submission`, async () => {
    const h = await submitHarness(mode, async () => ({ id: 'created' }));
    await h.submit();
    assert.equal(h.bindings.sessionCurrent, false);
    await h.submit();
    assert.equal(
      h.posts.length,
      1,
      'the completed render cannot publish the same captured document again',
    );
    assert.equal(h.resets.length, 1);
  });
}
