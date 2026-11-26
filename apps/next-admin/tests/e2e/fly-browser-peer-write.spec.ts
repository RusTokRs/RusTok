import { expect, test, type Page } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { openBlankDocument } from './support/fly-blank-document';

const adapterPath = resolve(
  process.cwd(),
  '../../crates/ui/fly/browser/assets/fly-browser.js'
);

type PeerScope = typeof globalThis & {
  __FLY_BROWSER_CONFIG__?: Record<string, unknown>;
  __flyPeerRequests?: string[];
  __flyPeerEmitted?: string[];
  __flyPeerAdapter?: {
    emitIntent: (intent: string, payload: Record<string, unknown>) => Promise<unknown>;
    stop: () => void;
    resolvePendingOperation: () => Promise<unknown>;
    retryPendingOperation: () => Promise<unknown>;
  };
  FlyBrowser?: {
    mount: (root: Element, options?: Record<string, unknown>) =>
      PeerScope['__flyPeerAdapter'];
  };
};

async function mountEditor(page: Page, documentId: string, source: string) {
  await openBlankDocument(page);
  await page.evaluate(async ({ documentId, source }) => {
    document.body.innerHTML = `
      <div id="fly-root" data-fly-browser-root
        data-fly-page-id="${documentId}"
        data-fly-expected-origin="null"
        data-fly-revision="rev-1"
        data-fly-project-hash="hash-1"
        data-fly-intent-endpoint="/fly-intent">
        <iframe id="fly-frame" title="Peer-write canvas" data-fly-iframe-canvas></iframe>
      </div>`;
    const scope = globalThis as PeerScope;
    scope.__FLY_BROWSER_CONFIG__ = { autoMount: false };
    scope.__flyPeerRequests = [];
    scope.__flyPeerEmitted = [];
    document.querySelector('#fly-root')?.addEventListener('fly:browser-intent', (event) => {
      scope.__flyPeerEmitted?.push((event as CustomEvent<{ intent: string }>).detail.intent);
    });
    const originalFetch = globalThis.fetch;
    globalThis.fetch = (input, init) => {
      const url = typeof input === 'string' ? input : String(input);
      if (!url.includes('/fly-intent')) return originalFetch(input, init);
      const request = JSON.parse(String(init?.body)) as { intent: string };
      scope.__flyPeerRequests?.push(request.intent);
      return Promise.resolve(new Response(JSON.stringify({
        result: {
          revision_id: 'rev-2',
          project_hash: 'hash-2',
          dirty: false
        }
      }), { status: 200, headers: { 'content-type': 'application/json' } }));
    };
    const url = URL.createObjectURL(new Blob([source], { type: 'text/javascript' }));
    try {
      await import(url);
    } finally {
      URL.revokeObjectURL(url);
    }
    const root = document.querySelector('#fly-root');
    if (!root) throw new Error('missing editor root');
    scope.__flyPeerAdapter = scope.FlyBrowser?.mount(root, {
      intentEndpoint: '/fly-intent'
    });
    if (!scope.__flyPeerAdapter) throw new Error('adapter did not mount');
  }, { documentId, source });
}

test('confirmed peer write fences stale tab without losing its local view or sending writes', async ({ context, page }) => {
  const source = await readFile(adapterPath, 'utf8');
  const otherTab = await context.newPage();
  await mountEditor(page, 'same-page', source);
  await mountEditor(otherTab, 'same-page', source);
  const [firstSender, secondSender] = await Promise.all([
    page.locator('#fly-root').getAttribute('data-fly-peer-write-sender-id'),
    otherTab.locator('#fly-root').getAttribute('data-fly-peer-write-sender-id'),
  ]);
  expect(firstSender).toBeTruthy();
  expect(secondSender).toBeTruthy();
  expect(firstSender).not.toBe(secondSender);

  await page.evaluate(async () => {
    await (globalThis as PeerScope).__flyPeerAdapter?.emitIntent('save', {});
  });
  await expect(otherTab.locator('#fly-root')).toHaveAttribute(
    'data-fly-peer-write-conflict', 'true'
  );
  await expect(otherTab.locator('#fly-root')).toHaveAttribute(
    'data-fly-browser-problem', 'FLY_PEER_WRITE_CONFLICT'
  );

  const state = await otherTab.evaluate(async () => {
    const scope = globalThis as PeerScope;
    const rejected = await scope.__flyPeerAdapter?.emitIntent('save', {});
    const read = await scope.__flyPeerAdapter?.emitIntent('select', {});
    return {
      rejected,
      readIsPresent: read !== null,
      posted: scope.__flyPeerRequests,
      emitted: scope.__flyPeerEmitted,
      conflict: document.querySelector('#fly-root')?.getAttribute('data-fly-peer-write-conflict'),
      problem: document.querySelector('#fly-root')?.getAttribute('data-fly-browser-problem'),
    };
  });
  expect(state).toEqual({
    rejected: null,
    readIsPresent: true,
    posted: ['select'],
    emitted: ['select'],
    conflict: 'true',
    problem: 'FLY_PEER_WRITE_CONFLICT'
  });
  await otherTab.evaluate(() => (globalThis as PeerScope).__flyPeerAdapter?.stop());
  await expect(otherTab.locator('#fly-root')).not.toHaveAttribute(
    'data-fly-peer-write-conflict', 'true'
  );
});

test('same-document Rust and JS channels advance one baseline without a false conflict', async ({ page }) => {
  const source = await readFile(adapterPath, 'utf8');
  await mountEditor(page, 'same-page', source);

  await page.evaluate(() => {
    const senderId = document.querySelector('#fly-root')
      ?.getAttribute('data-fly-peer-write-sender-id');
    const channel = new BroadcastChannel('fly:page-authoring-writes:v1');
    channel.postMessage({
      kind: 'confirmed-write',
      pageId: 'same-page',
      revision: 'rev-2',
      hash: 'hash-2',
      senderId,
    });
    channel.close();
  });
  await expect.poll(() => page.locator('#fly-root').getAttribute('data-fly-revision'))
    .toBe('rev-2');
  await expect(page.locator('#fly-root')).not.toHaveAttribute(
    'data-fly-peer-write-conflict', 'true'
  );

  await page.evaluate(() => {
    const channel = new BroadcastChannel('fly:page-authoring-writes:v1');
    channel.postMessage({
      kind: 'confirmed-write',
      pageId: 'same-page',
      revision: 'rev-3',
      hash: 'hash-3',
      senderId: 'another-document',
    });
    channel.close();
  });
  await expect(page.locator('#fly-root')).toHaveAttribute(
    'data-fly-peer-write-conflict', 'true'
  );
});

test('malformed peer notifications neither fence nor advance the editor baseline', async ({ page }) => {
  const source = await readFile(adapterPath, 'utf8');
  await mountEditor(page, 'same-page', source);

  await page.evaluate(() => {
    const senderId = document.querySelector('#fly-root')
      ?.getAttribute('data-fly-peer-write-sender-id');
    const channel = new BroadcastChannel('fly:page-authoring-writes:v1');
    const message = {
      kind: 'confirmed-write',
      pageId: 'same-page',
      revision: 'rev-invalid',
      hash: 'hash-invalid',
    };
    channel.postMessage(message);
    channel.postMessage({ ...message, senderId: ' \u0085' });
    channel.postMessage({ ...message, senderId: 'é'.repeat(129) });
    channel.postMessage({ ...message, senderId: 'peer', revision: 'é'.repeat(129) });
    channel.postMessage({ ...message, senderId: 'peer', extra: 'x'.repeat(1024) });
    channel.postMessage(JSON.stringify({ ...message, senderId: 'peer' }));
    // A same-sender write on the same channel acts as a delivery barrier:
    // all earlier notifications must have been processed before this one.
    channel.postMessage({ ...message, revision: 'rev-2', hash: 'hash-2', senderId });
    channel.close();
  });
  await expect(page.locator('#fly-root')).toHaveAttribute('data-fly-revision', 'rev-2');
  await expect(page.locator('#fly-root')).toHaveAttribute('data-fly-project-hash', 'hash-2');
  await expect(page.locator('#fly-root')).not.toHaveAttribute('data-fly-peer-write-conflict', 'true');

  await page.evaluate(() => {
    const channel = new BroadcastChannel('fly:page-authoring-writes:v1');
    channel.postMessage({
      kind: 'confirmed-write', pageId: 'same-page',
      revision: 'rev-3', hash: 'hash-3', senderId: 'peer',
    });
    channel.close();
  });
  await expect(page.locator('#fly-root')).toHaveAttribute('data-fly-peer-write-conflict', 'true');
});

test('server revision conflicts fence the browser editor without BroadcastChannel', async ({ page }) => {
  const source = await readFile(adapterPath, 'utf8');
  await mountEditor(page, 'same-page', source);

  await page.locator('#fly-root').evaluate((root) => {
    root.dispatchEvent(new CustomEvent('fly:browser-intent-response', {
      bubbles: true,
      detail: {
        ok: true,
        current: true,
        request: { intent: 'publish' },
        result: {
          save_failure: {
            code: 'REVISION_CONFLICT',
            message: 'The page revision changed in another session',
          },
        },
      },
    }));
  });

  await expect(page.locator('#fly-root')).toHaveAttribute('data-fly-peer-write-conflict', 'true');
  await expect(page.locator('#fly-root')).toHaveAttribute(
    'data-fly-browser-problem', 'FLY_PEER_WRITE_CONFLICT'
  );
});

test('a fresh server revision clears a persisted revision-conflict fence', async ({ page }) => {
  const source = await readFile(adapterPath, 'utf8');
  await mountEditor(page, 'same-page', source);

  await page.locator('#fly-root').evaluate((root) => {
    root.dispatchEvent(new CustomEvent('fly:browser-intent-response', {
      bubbles: true,
      detail: {
        ok: true,
        current: true,
        request: { intent: 'publish' },
        result: {
          save_failure: {
            code: 'REVISION_CONFLICT',
            message: 'The page revision changed in another session',
          },
        },
      },
    }));
  });
  await expect(page.locator('#fly-root')).toHaveAttribute('data-fly-peer-write-conflict', 'true');

  await page.evaluate(() => {
    const scope = globalThis as PeerScope;
    scope.__flyPeerAdapter?.stop();
    const root = document.querySelector('#fly-root') as HTMLElement | null;
    if (!root) throw new Error('missing editor root');
    root.dataset.flyRevision = 'rev-2';
    scope.__flyPeerAdapter = scope.FlyBrowser?.mount(root, {
      intentEndpoint: '/fly-intent',
    });
    if (!scope.__flyPeerAdapter) throw new Error('adapter did not remount');
  });

  await expect(page.locator('#fly-root')).not.toHaveAttribute(
    'data-fly-peer-write-conflict', 'true'
  );
  await expect(page.locator('#fly-root')).not.toHaveAttribute(
    'data-fly-browser-problem', 'FLY_PEER_WRITE_CONFLICT'
  );
});

test('peer write on another page does not fence unrelated editors', async ({ context, page }) => {
  const source = await readFile(adapterPath, 'utf8');
  const otherTab = await context.newPage();
  await mountEditor(page, 'page-one', source);
  await mountEditor(otherTab, 'page-two', source);
  await page.evaluate(async () => {
    await (globalThis as PeerScope).__flyPeerAdapter?.emitIntent('save', {});
  });
  const state = await otherTab.evaluate(async () => {
    const scope = globalThis as PeerScope;
    await scope.__flyPeerAdapter?.emitIntent('save', {});
    return {
      posted: scope.__flyPeerRequests,
      emitted: scope.__flyPeerEmitted,
      conflict: document.querySelector('#fly-root')?.getAttribute('data-fly-peer-write-conflict'),
    };
  });
  expect(state).toEqual({ posted: ['save'], emitted: ['save'], conflict: null });
});


test('HTTP revision conflict fences a stale editor before the next mutation', async ({ page }) => {
  const source = await readFile(adapterPath, 'utf8');
  await mountEditor(page, 'same-page', source);
  await page.evaluate(async () => {
    globalThis.fetch = async () => new Response(JSON.stringify({
      code: 'REVISION_CONFLICT', error: 'stale revision'
    }), { status: 409, headers: { 'content-type': 'application/json' } });
    await (globalThis as PeerScope).__flyPeerAdapter?.emitIntent('save', {});
  });
  await expect(page.locator('#fly-root')).toHaveAttribute('data-fly-peer-write-conflict', 'true');
  await expect(page.locator('#fly-root')).toHaveAttribute('data-fly-browser-problem', 'FLY_PEER_WRITE_CONFLICT');
});

test('an in-flight response cannot replace a peer-write fence', async ({ page }) => {
  const source = await readFile(adapterPath, 'utf8');
  await mountEditor(page, 'same-page', source);
  await page.evaluate(async () => {
    const scope = globalThis as PeerScope;
    const root = document.querySelector('#fly-root')!;
    let finish: (response: Response) => void = () => {};
    globalThis.fetch = () => new Promise<Response>((resolve) => { finish = resolve; });
    const pending = scope.__flyPeerAdapter?.emitIntent('save', {});
    const fenced = new Promise<void>((resolve) => root.addEventListener(
      'fly:browser-peer-write', () => resolve(), { once: true }
    ));
    const channel = new BroadcastChannel('fly:page-authoring-writes:v1');
    channel.postMessage({
      kind: 'confirmed-write', pageId: 'same-page',
      revision: 'rev-peer', hash: 'hash-peer', senderId: 'other-editor'
    });
    await fenced;
    channel.close();
    finish(new Response(JSON.stringify({ code: 'SAVE_FAILED', error: 'late error' }),
      { status: 500, headers: { 'content-type': 'application/json' } }));
    await pending;
  });
  await expect(page.locator('#fly-root')).toHaveAttribute('data-fly-browser-problem', 'FLY_PEER_WRITE_CONFLICT');
  await expect(page.locator('#fly-root')).toHaveAttribute('data-fly-revision', 'rev-1');
});


test('strict receipt recovery rejects concurrent retry and ignores an unknown receipt baseline', async ({ page }) => {
  const source = await readFile(adapterPath, 'utf8');
  await mountEditor(page, 'same-page', source);
  const result = await page.evaluate(async () => {
    const scope = globalThis as PeerScope;
    scope.__flyPeerAdapter?.stop();
    const root = document.querySelector('#fly-root') as HTMLElement;
    root.dataset.flyDraftContract = 'rendered';
    root.dataset.flyRenderedDraftToken = 'draft-1';
    sessionStorage.setItem('fly:ssr-draft:same-page', JSON.stringify({ token: 'draft-1', generation: 1 }));
    const original = {
      protocol: 'fly_iframe', instance_id: 'editor', intent: 'save', payload: {},
      page_id: 'same-page', revision: 'rev-1', project_hash: 'hash-1',
      draft_token: 'draft-1', draft_generation: 1, operation_id: 'operation-1'
    };
    const storageKey = 'fly:ssr-pending-operation:same-page';
    sessionStorage.setItem(storageKey, JSON.stringify({ at: Date.now(), request: original }));
    const requests: Array<{ intent: string; operation_id: string }> = [];
    const responses: Array<(response: Response) => void> = [];
    globalThis.fetch = (_input, init) => {
      requests.push(JSON.parse(String(init?.body)));
      return new Promise<Response>((resolve) => responses.push(resolve));
    };
    const adapter = scope.FlyBrowser?.mount(root, { intentEndpoint: '/fly-intent' });
    if (!adapter) throw new Error('missing adapter');
    scope.__flyPeerAdapter = adapter;
    await Promise.resolve(); // Mount schedules one recovery lookup.
    await adapter.resolvePendingOperation();
    await adapter.retryPendingOperation();
    await adapter.emitIntent('select', { component_id: 'hero' });
    if (requests.length !== 1) throw new Error('concurrent draft operation was posted');
    const responseEvent = new Promise<void>((resolve) => root.addEventListener(
      'fly:browser-intent-response', () => resolve(), { once: true }
    ));
    responses[0](new Response(JSON.stringify({
      operation_id: 'operation-1', operation_status: 'unknown',
      result: { revision_id: 'unconfirmed', project_hash: 'unconfirmed', dirty: false },
      draft_token: 'draft-1', draft_generation: 9
    }), { status: 200, headers: { 'content-type': 'application/json' } }));
    await responseEvent;
    const afterUnknown = {
      revision: root.dataset.flyRevision,
      draft: JSON.parse(sessionStorage.getItem('fly:ssr-draft:same-page')!),
      pending: JSON.parse(sessionStorage.getItem(storageKey)!).request.operation_id
    };
    const lookup = adapter.resolvePendingOperation();
    if (!requests[1]) throw new Error('second lookup was not sent');
    responses[1](new Response(JSON.stringify({
      operation_id: 'operation-1', operation_status: 'committed',
      result: { revision_id: 'rev-2', project_hash: 'hash-2', dirty: false },
      draft_token: 'draft-1', draft_generation: 2
    }), { status: 200, headers: { 'content-type': 'application/json' } }));
    await lookup;
    return {
      requests: requests.map(({ intent, operation_id }) => ({ intent, operation_id })),
      afterUnknown,
      pending: sessionStorage.getItem(storageKey),
      revision: root.dataset.flyRevision
    };
  });
  expect(result).toEqual({
    requests: [
      { intent: 'resolve_operation', operation_id: 'operation-1' },
      { intent: 'resolve_operation', operation_id: 'operation-1' }
    ],
    afterUnknown: { revision: 'rev-1', draft: { token: 'draft-1', generation: 1 }, pending: 'operation-1' },
    pending: null,
    revision: 'rev-2'
  });
});

test('strict draft view requests use the same receipt-ordered transport as edits', async ({ page }) => {
  const source = await readFile(adapterPath, 'utf8');
  await mountEditor(page, 'same-page', source);
  const observed = await page.evaluate(async () => {
    const scope = globalThis as PeerScope;
    scope.__flyPeerAdapter?.stop();
    const root = document.querySelector('#fly-root') as HTMLElement;
    root.dataset.flyDraftContract = 'rendered';
    root.dataset.flyRenderedDraftToken = 'draft-1';
    sessionStorage.setItem('fly:ssr-draft:same-page', JSON.stringify({ token: 'draft-1', generation: 1 }));
    const requests: Array<{ intent: string; draft_generation: number; operation_id: string }> = [];
    let finish: (response: Response) => void = () => {};
    globalThis.fetch = (_input, init) => {
      const request = JSON.parse(String(init?.body));
      requests.push(request);
      return new Promise<Response>((resolve) => { finish = resolve; });
    };
    const adapter = scope.FlyBrowser?.mount(root, { intentEndpoint: '/fly-intent' });
    if (!adapter) throw new Error('missing adapter');
    const select = adapter.emitIntent('select', { component_id: 'hero' });
    const save = adapter.emitIntent('save', {});
    const before = requests.length;
    const reply = (generation: number) => new Response(JSON.stringify({
      operation_id: requests[requests.length - 1].operation_id,
      operation_status: 'committed',
      result: { revision_id: 'rev-1', project_hash: 'hash-1', dirty: false },
      draft_token: 'draft-1', draft_generation: generation
    }), { status: 200, headers: { 'content-type': 'application/json' } });
    finish(reply(2));
    await select;
    finish(reply(3));
    await save;
    return {
      before,
      requests: requests.map(({ intent, draft_generation, operation_id }) =>
        ({ intent, draft_generation, hasReceipt: Boolean(operation_id) }))
    };
  });
  expect(observed).toEqual({
    before: 1,
    requests: [
      { intent: 'select', draft_generation: 1, hasReceipt: true },
      { intent: 'save', draft_generation: 2, hasReceipt: true }
    ]
  });
});


test('a persisted write requiring SSR reload still notifies peer editors', async ({ context, page }) => {
  const source = await readFile(adapterPath, 'utf8');
  const peer = await context.newPage();
  await mountEditor(page, 'same-page', source);
  await mountEditor(peer, 'same-page', source);
  await page.evaluate(() => {
    globalThis.fetch = async () => new Response(JSON.stringify({
      reload: true,
      result: { revision_id: 'rev-2', project_hash: 'hash-2', dirty: false }
    }), { status: 200, headers: { 'content-type': 'application/json' } });
    void (globalThis as PeerScope).__flyPeerAdapter?.emitIntent('save', {});
  });
  await expect(peer.locator('#fly-root')).toHaveAttribute('data-fly-peer-write-conflict', 'true');
  await peer.close();
});

test('dirty draft-only responses do not report a confirmed peer write', async ({ context, page }) => {
  const source = await readFile(adapterPath, 'utf8');
  const peer = await context.newPage();
  await mountEditor(page, 'same-page', source);
  await mountEditor(peer, 'same-page', source);
  const broadcasts = await page.evaluate(async () => {
    const sent: unknown[] = [];
    const original = BroadcastChannel.prototype.postMessage;
    BroadcastChannel.prototype.postMessage = function (message: unknown) {
      sent.push(message);
      original.call(this, message);
    };
    try {
      globalThis.fetch = async () => new Response(JSON.stringify({
        result: { revision_id: 'rev-1', project_hash: 'dirty-hash', dirty: true }
      }), { status: 200, headers: { 'content-type': 'application/json' } });
      await (globalThis as PeerScope).__flyPeerAdapter?.emitIntent('save', {});
      return sent;
    } finally {
      BroadcastChannel.prototype.postMessage = original;
    }
  });
  expect(broadcasts).toEqual([]);
  await expect(peer.locator('#fly-root')).not.toHaveAttribute('data-fly-peer-write-conflict', 'true');
  await peer.close();
});

test('strict success without a matching receipt cannot rebase or navigate the editor', async ({ page }) => {
  const source = await readFile(adapterPath, 'utf8');
  await mountEditor(page, 'same-page', source);
  const observed = await page.evaluate(async () => {
    const scope = globalThis as PeerScope;
    scope.__flyPeerAdapter?.stop();
    const root = document.querySelector('#fly-root') as HTMLElement;
    root.dataset.flyDraftContract = 'rendered';
    root.dataset.flyRenderedDraftToken = 'draft-1';
    const draftKey = 'fly:ssr-draft:same-page';
    const pendingKey = 'fly:ssr-pending-operation:same-page';
    sessionStorage.setItem(draftKey, JSON.stringify({ token: 'draft-1', generation: 1 }));
    const requests: string[] = [];
    const emitted: string[] = [];
    const current: boolean[] = [];
    root.addEventListener('fly:browser-intent', (event) => {
      emitted.push((event as CustomEvent<{ intent: string }>).detail.intent);
    });
    root.addEventListener('fly:browser-intent-response', (event) => {
      current.push((event as CustomEvent<{ current: boolean }>).detail.current);
    });
    globalThis.fetch = (_input, init) => {
      const request = JSON.parse(String(init?.body)) as { intent: string };
      requests.push(request.intent);
      if (request.intent === 'resolve_operation') {
        return new Promise<Response>(() => {});
      }
      return Promise.resolve(new Response(JSON.stringify({
        operation_id: 'wrong-operation',
        operation_status: 'committed',
        result: { revision_id: 'wrong-revision', project_hash: 'wrong-hash', dirty: false },
        draft_token: 'wrong-draft', draft_generation: 99,
        location: '#unconfirmed'
      }), { status: 200, headers: { 'content-type': 'application/json' } }));
    };
    const adapter = scope.FlyBrowser?.mount(root, { intentEndpoint: '/fly-intent' });
    if (!adapter) throw new Error('missing adapter');
    const beforeLocation = location.href;
    await adapter.emitIntent('save', {});
    await adapter.emitIntent('select', { component_id: 'hero' });
    const state = {
      requests, emitted, current,
      revision: root.dataset.flyRevision,
      hash: root.dataset.flyProjectHash,
      draft: JSON.parse(sessionStorage.getItem(draftKey)!),
      pending: Boolean(sessionStorage.getItem(pendingKey)),
      locationUnchanged: location.href === beforeLocation
    };
    adapter.stop();
    return state;
  });
  expect(observed).toEqual({
    requests: ['save', 'resolve_operation'],
    emitted: ['save'],
    current: [false],
    revision: 'rev-1',
    hash: 'hash-1',
    draft: { token: 'draft-1', generation: 1 },
    pending: true,
    locationUnchanged: true
  });
});

test('strict peer fence rejects view-state intents before emitting them', async ({ page }) => {
  const source = await readFile(adapterPath, 'utf8');
  await mountEditor(page, 'same-page', source);
  const observed = await page.evaluate(async () => {
    const scope = globalThis as PeerScope;
    scope.__flyPeerAdapter?.stop();
    const root = document.querySelector('#fly-root') as HTMLElement;
    root.dataset.flyDraftContract = 'rendered';
    root.dataset.flyRenderedDraftToken = 'draft-1';
    sessionStorage.setItem('fly:ssr-draft:same-page', JSON.stringify({ token: 'draft-1', generation: 1 }));
    const adapter = scope.FlyBrowser?.mount(root, { intentEndpoint: '/fly-intent' });
    if (!adapter) throw new Error('missing adapter');
    const fenced = new Promise<void>((resolve) => root.addEventListener(
      'fly:browser-peer-write', () => resolve(), { once: true }
    ));
    const channel = new BroadcastChannel('fly:page-authoring-writes:v1');
    channel.postMessage({
      kind: 'confirmed-write', pageId: 'same-page',
      revision: 'rev-peer', hash: 'hash-peer', senderId: 'other-editor'
    });
    await fenced;
    channel.close();
    await adapter.emitIntent('select', { component_id: 'hero' });
    return { posted: scope.__flyPeerRequests, emitted: scope.__flyPeerEmitted };
  });
  expect(observed).toEqual({ posted: [], emitted: [] });
});


test('strict pending storage cannot be overwritten by another mounted root or cleared by an older response', async ({ page }) => {
  const source = await readFile(adapterPath, 'utf8');
  await mountEditor(page, 'same-page', source);
  const observed = await page.evaluate(async () => {
    const scope = globalThis as PeerScope;
    scope.__flyPeerAdapter?.stop();
    const root = document.querySelector('#fly-root') as HTMLElement;
    root.dataset.flyDraftContract = 'rendered';
    root.dataset.flyRenderedDraftToken = 'draft-1';
    const draftKey = 'fly:ssr-draft:same-page';
    const pendingKey = 'fly:ssr-pending-operation:same-page';
    sessionStorage.setItem(draftKey, JSON.stringify({ token: 'draft-1', generation: 1 }));
    const otherRoot = root.cloneNode(true) as HTMLElement;
    otherRoot.id = 'other-root';
    otherRoot.querySelector('iframe')!.id = 'other-frame';
    document.body.append(otherRoot);
    const requests: Array<{ operation_id: string; intent: string }> = [];
    let finish: (response: Response) => void = () => {};
    globalThis.fetch = (_input, init) => {
      requests.push(JSON.parse(String(init?.body)));
      return new Promise<Response>((resolve) => { finish = resolve; });
    };
    const first = scope.FlyBrowser?.mount(root, { intentEndpoint: '/fly-intent' });
    const second = scope.FlyBrowser?.mount(otherRoot, { intentEndpoint: '/fly-intent' });
    if (!first || !second) throw new Error('missing adapters');
    const firstSave = first.emitIntent('save', {});
    const parked = sessionStorage.getItem(pendingKey)!;
    const emitted: string[] = [];
    otherRoot.addEventListener('fly:browser-intent', (event) => {
      emitted.push((event as CustomEvent<{ intent: string }>).detail.intent);
    });
    await second.emitIntent('save', {});
    const preserved = sessionStorage.getItem(pendingKey) === parked;
    // Simulate a different mount having resolved the first operation and
    // reserved its successor before the original HTTP response arrives.
    const successor = JSON.parse(parked);
    successor.request.operation_id = 'successor';
    successor.request.draft_generation = 2;
    sessionStorage.setItem(pendingKey, JSON.stringify(successor));
    finish(new Response(JSON.stringify({
      operation_id: requests[0].operation_id, operation_status: 'committed',
      result: { revision_id: 'rev-1', project_hash: 'hash-1', dirty: false },
      draft_token: 'draft-1', draft_generation: 2
    }), { status: 200, headers: { 'content-type': 'application/json' } }));
    await firstSave;
    const after = JSON.parse(sessionStorage.getItem(pendingKey)!).request.operation_id;
    await second.retryPendingOperation();
    first.stop();
    second.stop();
    return { preserved, after, emitted, posted: requests.map((request) => request.intent) };
  });
  expect(observed).toEqual({ preserved: true, after: 'successor', emitted: [], posted: ['save'] });
});

test('a matching operation ID with a different draft or unchanged generation is not a receipt', async ({ page }) => {
  const source = await readFile(adapterPath, 'utf8');
  await mountEditor(page, 'same-page', source);
  const observed = await page.evaluate(async () => {
    const scope = globalThis as PeerScope;
    scope.__flyPeerAdapter?.stop();
    const root = document.querySelector('#fly-root') as HTMLElement;
    root.dataset.flyDraftContract = 'rendered';
    root.dataset.flyRenderedDraftToken = 'draft-1';
    const key = 'fly:ssr-pending-operation:same-page';
    sessionStorage.setItem('fly:ssr-draft:same-page', JSON.stringify({ token: 'draft-1', generation: 1 }));
    const responses: Array<(response: Response) => void> = [];
    const requests: Array<{ operation_id: string }> = [];
    globalThis.fetch = (_input, init) => {
      requests.push(JSON.parse(String(init?.body)));
      return new Promise<Response>((resolve) => responses.push(resolve));
    };
    const adapter = scope.FlyBrowser?.mount(root, { intentEndpoint: '/fly-intent' });
    if (!adapter) throw new Error('missing adapter');
    const reply = (token: string, generation: number) => new Response(JSON.stringify({
      operation_id: requests[0].operation_id, operation_status: 'committed',
      result: { revision_id: 'rev-wrong', project_hash: 'hash-wrong', dirty: false },
      draft_token: token, draft_generation: generation
    }), { status: 200, headers: { 'content-type': 'application/json' } });
    const save = adapter.emitIntent('save', {});
    responses[0](reply('different-draft', 2));
    const result = await save;
    await Promise.resolve();
    const lookup = new Promise<void>((resolve) => root.addEventListener(
      'fly:browser-intent-response', () => resolve(), { once: true }
    ));
    responses[1](reply('draft-1', 1));
    await lookup;
    const state = { result, pending: Boolean(sessionStorage.getItem(key)),
      revision: root.dataset.flyRevision, requests: requests.length };
    adapter.stop();
    return state;
  });
  expect(observed).toEqual({ result: null, pending: true, revision: 'rev-1', requests: 2 });
});
