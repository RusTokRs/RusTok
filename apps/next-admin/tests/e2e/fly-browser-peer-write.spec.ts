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
  __flyPeerResolveIntent?: () => void;
  __flyPeerAdapter?: {
    emitIntent: (intent: string, payload: Record<string, unknown>) => Promise<unknown>;
    stop: () => void;
  };
  FlyBrowser?: {
    mount: (root: Element, options?: Record<string, unknown>) =>
      PeerScope['__flyPeerAdapter'];
  };
};

type MountOptions = {
  initialRevision?: string;
  initialHash?: string;
  responseRevision?: string;
  responseHash?: string;
  deferSaveResponse?: boolean;
};

async function mountEditor(
  page: Page,
  documentId: string,
  source: string,
  options: MountOptions = {}
) {
  await openBlankDocument(page);
  await page.evaluate(async ({ documentId, source, options }) => {
    const initialRevision = options.initialRevision ?? 'rev-1';
    const initialHash = options.initialHash ?? 'hash-1';
    const responseRevision = options.responseRevision ?? 'rev-2';
    const responseHash = options.responseHash ?? 'hash-2';
    document.body.innerHTML = `
      <div id="fly-root" data-fly-browser-root
        data-fly-page-id="${documentId}"
        data-fly-expected-origin="null"
        data-fly-revision="${initialRevision}"
        data-fly-project-hash="${initialHash}"
        data-fly-intent-endpoint="/fly-intent">
        <iframe id="fly-frame" title="Peer-write canvas" data-fly-iframe-canvas></iframe>
      </div>`;
    const scope = globalThis as PeerScope;
    scope.__FLY_BROWSER_CONFIG__ = { autoMount: false };
    scope.__flyPeerRequests = [];
    scope.__flyPeerEmitted = [];
    scope.__flyPeerResolveIntent = undefined;
    document.querySelector('#fly-root')?.addEventListener('fly:browser-intent', (event) => {
      scope.__flyPeerEmitted?.push((event as CustomEvent<{ intent: string }>).detail.intent);
    });
    const originalFetch = globalThis.fetch;
    globalThis.fetch = (input, init) => {
      const url = typeof input === 'string' ? input : String(input);
      if (!url.includes('/fly-intent')) return originalFetch(input, init);
      const request = JSON.parse(String(init?.body)) as { intent: string };
      scope.__flyPeerRequests?.push(request.intent);
      const response = () => new Response(JSON.stringify({
        result: {
          revision_id: responseRevision,
          project_hash: responseHash,
          dirty: false
        }
      }), { status: 200, headers: { 'content-type': 'application/json' } });
      if (options.deferSaveResponse && request.intent === 'save') {
        return new Promise<Response>((resolve) => {
          scope.__flyPeerResolveIntent = () => resolve(response());
        });
      }
      return Promise.resolve(response());
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
  }, { documentId, source, options });
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

test('a fenced in-flight save does not broadcast an older revision to the winning tab', async ({ context, page }) => {
  const source = await readFile(adapterPath, 'utf8');
  const winningTab = await context.newPage();
  await mountEditor(page, 'same-page', source, {
    initialRevision: 'rev-1',
    initialHash: 'hash-1',
    responseRevision: 'rev-2',
    responseHash: 'hash-2',
    deferSaveResponse: true,
  });
  await mountEditor(winningTab, 'same-page', source, {
    initialRevision: 'rev-2',
    initialHash: 'hash-2',
    responseRevision: 'rev-3',
    responseHash: 'hash-3',
  });

  const pendingSave = page.evaluate(async () => {
    await (globalThis as PeerScope).__flyPeerAdapter?.emitIntent('save', {});
  });
  await expect.poll(() => page.evaluate(
    () => Boolean((globalThis as PeerScope).__flyPeerResolveIntent)
  )).toBe(true);

  await winningTab.evaluate(async () => {
    await (globalThis as PeerScope).__flyPeerAdapter?.emitIntent('save', {});
  });
  await expect(page.locator('#fly-root')).toHaveAttribute(
    'data-fly-peer-write-conflict', 'true'
  );

  await page.evaluate(() => {
    const scope = globalThis as PeerScope;
    scope.__flyPeerResolveIntent?.();
    scope.__flyPeerResolveIntent = undefined;
  });
  await pendingSave;

  await expect(winningTab.locator('#fly-root')).not.toHaveAttribute(
    'data-fly-peer-write-conflict', 'true'
  );
});

test('same-document Rust and JS channels advance one baseline without a false conflict', async ({ page }) => {
  const source = await readFile(adapterPath, 'utf8');
  await mountEditor(page, 'same-page', source);

  await page.evaluate(() => {
    const senderId = document.querySelector('#fly-root')
      ?.getAttribute('data-fly-peer-write-sender-id');
    const channel = new BroadcastChannel('fly:page-authoring-writes');
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
    const channel = new BroadcastChannel('fly:page-authoring-writes');
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

test('server revision conflicts fence the browser editor without BroadcastChannel', async ({ page }) => {
  const source = await readFile(adapterPath, 'utf8');
  await mountEditor(page, 'same-page', source);

  await page.locator('#fly-root').evaluate((root) => {
    root.dispatchEvent(new CustomEvent('fly:browser-intent-response', {
      bubbles: true,
      detail: {
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
