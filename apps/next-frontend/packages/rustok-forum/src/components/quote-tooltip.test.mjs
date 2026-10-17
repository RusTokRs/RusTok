import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const tooltip = readFileSync(
  new URL('./quote-tooltip.tsx', import.meta.url),
  'utf8',
);
const section = readFileSync(
  new URL('./forum-section.tsx', import.meta.url),
  'utf8',
);
const detail = readFileSync(
  new URL('./topic-detail-view.tsx', import.meta.url),
  'utf8',
);

test('a quote from a closed composer opens a reply to the rendered topic before fetching its revision', () => {
  const click = tooltip.slice(tooltip.indexOf('async function onQuoteClick()'));
  assert.match(
    click,
    /if \(!state\.isOpen\) \{[\s\S]*?if \(!replyTopic\) return;[\s\S]*?openReply\(\{[\s\S]*?topicId: replyTopic\.id,[\s\S]*?topicTitle: replyTopic\.title,/,
  );
  assert.match(
    click,
    /const parentReplyId = targetContext\.targetKind === 'REPLY' \? targetContext\.targetId : null/,
  );
  assert.ok(
    click.indexOf('openReply({') <
      click.indexOf('await fetchStorefrontQuoteRevision('),
  );
  assert.match(
    click,
    /current\.isOpen && !current\.isSubmitting &&[\s\S]*?current\.parentReplyId === expectedTarget\.parentReplyId/,
  );
  assert.match(
    click,
    /if \(stillEditingTarget\(\)\) \{[\s\S]*?appendQuote\(null, snippet, authorHandle\)/,
  );
  assert.match(
    section,
    /<QuoteTooltip[^>]*replyTopic=\{selectedTopic && canReplyToTopic\(selectedTopic\)/,
  );
  assert.match(detail, /<QuoteTooltip[^>]*replyTopic=\{canReply \?/);
  assert.match(
    tooltip,
    /position && targetContext && \(state\.isOpen \|\| replyTopic !== null\)/,
  );
});

async function quoteClick(bindings) {
  const { stripTypeScriptTypes } = await import('node:module');
  const { compileFunction } = await import('node:vm');
  const start = tooltip.indexOf('  async function onQuoteClick()');
  const finish = tooltip.indexOf('\n\n  return (', start);
  assert.ok(start >= 0 && finish > start);
  return compileFunction(
    `${stripTypeScriptTypes(tooltip.slice(start, finish))}\nreturn onQuoteClick;`,
    Object.keys(bindings),
  )(...Object.values(bindings));
}

function quoteBindings(opened) {
  const calls = [];
  return {
    calls,
    state: {
      isOpen: false,
      isSubmitting: false,
      mode: 'reply',
      topicId: 'old-topic',
      categoryId: null,
      parentReplyId: null,
    },
    targetContext: { targetKind: 'REPLY', targetId: 'selected-reply' },
    quoteInFlight: { current: false },
    quoteActiveRef: { current: true },
    selectionVersionRef: { current: 0 },
    composerTarget: { current: {} },
    getClientAuth: () => ({ token: 'signed', tenant: 'shop' }),
    replyTopic: { id: 'rendered-topic', title: 'Rendered' },
    openReply: () => opened,
    captureEditorSession: () => () => true,
    tenantId: 'tenant-id',
    tenantSlug: 'shop',
    locale: 'en',
    selectedText: 'selected text',
    fetchStorefrontQuoteRevision: async () => {
      calls.push('lookup');
      return null;
    },
    appendQuote: (...args) => calls.push(args),
    setIsQuoting: () => {},
    setTextOnlyNotice: () => {},
    setPosition: () => {},
    window: { getSelection: () => ({ removeAllRanges() {} }) },
  };
}

test('tooltip lifecycle and quote opening are wired to the cancellation/acknowledgement guards', () => {
  assert.match(tooltip, /return \(\) => \{ quoteActiveRef.current = false;/);
  assert.match(tooltip, /if \(!openReply\([\s\S]*?\)\) return;/);
  assert.match(
    tooltip,
    /const handleSelectionChange = useCallback\(\(\) => \{\s*selectionVersionRef.current \+= 1;/,
  );
  assert.match(
    tooltip,
    /return quoteActiveRef.current && isEditingSession\(\) && currentAuth.token/,
  );
});

test('failed draft restoration cannot be treated as a successful quote target switch', async () => {
  const b = quoteBindings(false);
  await (
    await quoteClick(b)
  )();
  assert.deepEqual(
    b.calls,
    [],
    'do not fetch or append to the previous draft when opening failed',
  );
  assert.deepEqual(b.composerTarget.current, {});
});

test('an acknowledged reply target still accepts the captured text-only quote', async () => {
  const b = quoteBindings(true);
  await (
    await quoteClick(b)
  )();
  assert.deepEqual(b.calls, ['lookup', [null, 'selected text', undefined]]);
  assert.equal(b.composerTarget.current.topicId, 'rendered-topic');
});

test('a quote lookup finishing after tooltip unmount cannot append to a live composer', async () => {
  const b = quoteBindings(true);
  let resolve;
  b.fetchStorefrontQuoteRevision = () =>
    new Promise((yes) => {
      resolve = yes;
    });
  const pending = (await quoteClick(b))();
  b.quoteActiveRef.current = false;
  resolve(null);
  await pending;
  assert.deepEqual(b.calls, []);
});

for (const outcome of ['revision', 'fallback']) {
  test(`a late ${outcome} quote cannot enter a reopened editor with the same target`, async () => {
    const b = quoteBindings(true);
    const cleanups = [];
    b.window.getSelection = () => ({
      removeAllRanges: () => cleanups.push('selection'),
    });
    b.setPosition = () => cleanups.push('position');
    b.state.isOpen = true;
    b.composerTarget.current = { ...b.state };
    let generation = 1;
    b.captureEditorSession = () => {
      const captured = generation;
      return () => captured === generation;
    };
    let finish;
    b.fetchStorefrontQuoteRevision = () =>
      new Promise((resolve, reject) => {
        finish =
          outcome === 'revision'
            ? () => resolve('revision')
            : () => reject(new Error('offline'));
      });
    const pending = (await quoteClick(b))();
    generation += 2; // Successful close and reopen: same IDs, different editing session.
    finish();
    await pending;
    assert.deepEqual(
      cleanups,
      [],
      'late cleanup must not clear the reopened editor’s selection',
    );
    assert.deepEqual(
      b.calls,
      [],
      'closing cancels the original quote, even after reopening the same target',
    );
  });
}

test('quote completion cannot race the synchronous submit lock before a pending-state render', async () => {
  const b = quoteBindings(true);
  b.composerTarget.current = { ...b.state };
  let submitting = false;
  b.captureEditorSession = () => () => !submitting;
  let finish;
  b.fetchStorefrontQuoteRevision = () =>
    new Promise((resolve) => {
      finish = resolve;
    });
  const pending = (await quoteClick(b))();
  submitting = true;
  // composerTarget still describes the previous render, not the synchronous lock.
  assert.equal(b.composerTarget.current.isSubmitting, false);
  finish(null);
  await pending;
  assert.deepEqual(
    b.calls,
    [],
    'do not append text that was absent from the captured submission',
  );
});

test('finishing an earlier quote preserves a newer text selection and releases the quote lock', async () => {
  const b = quoteBindings(true);
  const cleanups = [],
    pendingStates = [];
  b.window.getSelection = () => ({
    removeAllRanges: () => cleanups.push('selection'),
  });
  b.setPosition = () => cleanups.push('position');
  b.setIsQuoting = (value) => pendingStates.push(value);
  let finish;
  b.fetchStorefrontQuoteRevision = () =>
    new Promise((resolve) => {
      finish = resolve;
    });
  const pending = (await quoteClick(b))();
  b.selectionVersionRef.current++;
  finish(null);
  await pending;
  assert.deepEqual(
    b.calls,
    [[null, 'selected text', undefined]],
    'the original quote still uses its captured text',
  );
  assert.deepEqual(
    cleanups,
    [],
    'do not erase a later selection or hide its quote action',
  );
  assert.deepEqual(pendingStates, [true, false]);
  assert.equal(b.quoteInFlight.current, false);
});
