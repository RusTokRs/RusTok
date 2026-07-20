import { expect, Page, test } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const adapterPath = resolve(
  process.cwd(),
  '../../crates/fly-browser/assets/fly-browser.js'
);

async function mountAdapter(page: Page) {
  const adapterSource = await readFile(adapterPath, 'utf8');
  await page.setContent(`
    <div
      id="fly-root"
      data-fly-browser-root
      data-fly-page-id="home"
      data-fly-revision="rev-1"
      data-fly-project-hash="hash-1"
      data-fly-expected-origin="null"
      data-fly-intent-endpoint="/fly-intent"
      style="position:relative"
    >
      <iframe
        id="canvas-a-frame"
        data-fly-iframe-canvas
        srcdoc="<!doctype html><html><body>canvas</body></html>"
      ></iframe>
    </div>
    <iframe id="foreign-frame" srcdoc="<!doctype html><html><body>foreign</body></html>"></iframe>
  `);

  await page.evaluate(async (source) => {
    const root = document.querySelector('#fly-root');
    if (!(root instanceof HTMLElement)) throw new Error('Fly root unavailable');
    const events: unknown[] = [];
    root.addEventListener('fly:browser-ready', (event) => {
      events.push({
        type: event.type,
        instanceId: (event as CustomEvent).detail?.instanceId
      });
    });
    root.addEventListener('fly:canvas-message', (event) => {
      events.push({
        type: event.type,
        sequence: (event as CustomEvent).detail?.sequence,
        messageType: (event as CustomEvent).detail?.message?.type
      });
    });
    root.addEventListener('fly:browser-intent-response', (event) => {
      const detail = (event as CustomEvent).detail;
      events.push({
        type: event.type,
        ok: detail?.ok,
        status: detail?.status,
        code: detail?.result?.code,
        intent: detail?.request?.intent,
        revision: detail?.request?.revision,
        projectHash: detail?.request?.project_hash
      });
    });
    root.addEventListener('fly:browser-error', (event) => {
      events.push({
        type: event.type,
        error: (event as CustomEvent).detail?.error
      });
    });
    (globalThis as typeof globalThis & { __flyEvents?: unknown[] }).__flyEvents =
      events;

    const url = URL.createObjectURL(
      new Blob([source], { type: 'text/javascript' })
    );
    try {
      await import(url);
    } finally {
      URL.revokeObjectURL(url);
    }
  }, adapterSource);

  await expect(page.locator('#fly-root')).toHaveAttribute(
    'data-fly-browser-mounted',
    'true'
  );
}

type BrowserMessage = Record<string, unknown> & { type: string };
type MockBrowserResponse = {
  status: number;
  body: Record<string, unknown>;
};

async function dispatchCanvasMessage(
  page: Page,
  message: BrowserMessage,
  options: {
    origin?: string;
    sequence?: number;
    source?: 'canvas' | 'foreign';
    instanceId?: string;
  } = {}
) {
  await page.evaluate(
    ({ message, origin, sequence, source, instanceId }) => {
      const frame = document.querySelector(
        source === 'foreign' ? '#foreign-frame' : '#canvas-a-frame'
      );
      if (!(frame instanceof HTMLIFrameElement) || !frame.contentWindow) {
        throw new Error('message source iframe unavailable');
      }
      window.dispatchEvent(
        new MessageEvent('message', {
          data: JSON.stringify({
            protocol: 'fly_iframe',
            instance_id: instanceId ?? 'canvas-a',
            sequence: sequence ?? 1,
            message
          }),
          origin: origin ?? 'null',
          source: frame.contentWindow
        })
      );
    },
    {
      message,
      origin: options.origin,
      sequence: options.sequence,
      source: options.source,
      instanceId: options.instanceId
    }
  );
}

async function installMockFetch(
  page: Page,
  responses: MockBrowserResponse[]
) {
  await page.evaluate((configuredResponses) => {
    const requests: unknown[] = [];
    let responseIndex = 0;
    (
      globalThis as typeof globalThis & {
        __flyRequests?: unknown[];
      }
    ).__flyRequests = requests;
    globalThis.fetch = async (input, init = {}) => {
      const body =
        typeof init.body === 'string' ? JSON.parse(init.body) : init.body ?? null;
      requests.push({
        input: String(input),
        method: init.method ?? 'GET',
        headers: init.headers ?? {},
        body
      });
      const response =
        configuredResponses[
          Math.min(responseIndex, configuredResponses.length - 1)
        ];
      responseIndex += 1;
      if (!response) throw new Error('mock response is unavailable');
      return new Response(JSON.stringify(response.body), {
        status: response.status,
        headers: { 'content-type': 'application/json' }
      });
    };
  }, responses);
}

async function emitBrowserIntent(
  page: Page,
  intent: string,
  payload: Record<string, unknown> = {}
) {
  return page.evaluate(
    async ({ intent, payload }) => {
      const root = document.querySelector('#fly-root');
      if (!(root instanceof HTMLElement)) throw new Error('Fly root unavailable');
      const flyBrowser = (
        globalThis as typeof globalThis & {
          FlyBrowser?: {
            mount?: (
              root: Element
            ) => {
              emitIntent: (
                intent: string,
                payload: Record<string, unknown>
              ) => Promise<unknown>;
            } | null;
          };
        }
      ).FlyBrowser;
      const adapter = flyBrowser?.mount?.(root);
      if (!adapter) throw new Error('Fly browser adapter unavailable');
      return adapter.emitIntent(intent, payload);
    },
    { intent, payload }
  );
}

test('mount handshake exposes one browser adapter instance', async ({ page }) => {
  await mountAdapter(page);

  const state = await page.evaluate(() => ({
    events: (
      globalThis as typeof globalThis & { __flyEvents?: unknown[] }
    ).__flyEvents,
    mounted: document.querySelector('#fly-root')?.getAttribute(
      'data-fly-browser-mounted'
    ),
    overlayCount: document.querySelectorAll('[data-fly-browser-overlay]')
      .length
  }));

  expect(state.events).toEqual([
    { type: 'fly:browser-ready', instanceId: 'canvas-a' }
  ]);
  expect(state.mounted).toBe('true');
  expect(state.overlayCount).toBe(3);
});

test('canvas messages require the expected source, origin and instance', async ({
  page
}) => {
  await mountAdapter(page);

  await dispatchCanvasMessage(page, { type: 'ready' }, { origin: 'https://evil.example' });
  await dispatchCanvasMessage(page, { type: 'ready' }, { source: 'foreign' });
  await dispatchCanvasMessage(page, { type: 'ready' }, { instanceId: 'canvas-b' });

  await expect(page.locator('#fly-root')).not.toHaveAttribute(
    'data-fly-canvas-connected',
    'true'
  );
  expect(
    await page.evaluate(
      () =>
        (
          globalThis as typeof globalThis & { __flyEvents?: unknown[] }
        ).__flyEvents?.filter(
          (event) =>
            (event as { type?: string }).type === 'fly:canvas-message'
        ) ?? []
    )
  ).toEqual([]);

  await dispatchCanvasMessage(page, { type: 'ready' }, { sequence: 4 });
  await expect(page.locator('#fly-root')).toHaveAttribute(
    'data-fly-canvas-connected',
    'true'
  );
});

test('sequence ordering rejects replayed teardown messages', async ({ page }) => {
  await mountAdapter(page);

  await dispatchCanvasMessage(page, { type: 'ready' }, { sequence: 5 });
  await dispatchCanvasMessage(page, { type: 'teardown' }, { sequence: 4 });
  await expect(page.locator('#fly-root')).toHaveAttribute(
    'data-fly-canvas-connected',
    'true'
  );

  await dispatchCanvasMessage(page, { type: 'teardown' }, { sequence: 6 });
  await expect(page.locator('#fly-root')).toHaveAttribute(
    'data-fly-canvas-connected',
    'false'
  );
});

test('geometry and viewport messages drive scaled selection overlays', async ({
  page
}) => {
  await mountAdapter(page);

  await dispatchCanvasMessage(
    page,
    {
      type: 'geometry_snapshot',
      components: [
        {
          component_id: 'hero',
          rect: { left: 10, top: 20, width: 30, height: 40 }
        }
      ]
    },
    { sequence: 1 }
  );
  await dispatchCanvasMessage(
    page,
    { type: 'viewport_changed', zoom: 2 },
    { sequence: 2 }
  );
  await dispatchCanvasMessage(
    page,
    { type: 'focus_requested', component_id: 'hero' },
    { sequence: 3 }
  );

  const overlay = page.locator('[data-fly-browser-overlay="selected"]');
  await expect(overlay).toHaveCSS('display', 'block');
  await expect(overlay).toHaveCSS('left', '20px');
  await expect(overlay).toHaveCSS('top', '40px');
  await expect(overlay).toHaveCSS('width', '60px');
  await expect(overlay).toHaveCSS('height', '80px');

  await dispatchCanvasMessage(page, { type: 'teardown' }, { sequence: 4 });
  await expect(overlay).toHaveCSS('display', 'none');
});

test('unmount removes overlays, listeners and connected state', async ({ page }) => {
  await mountAdapter(page);
  await dispatchCanvasMessage(page, { type: 'ready' }, { sequence: 1 });

  await page.evaluate(() => {
    const flyBrowser = (
      globalThis as typeof globalThis & {
        FlyBrowser?: { unmountAll?: () => void };
      }
    ).FlyBrowser;
    flyBrowser?.unmountAll?.();
  });

  const root = page.locator('#fly-root');
  await expect(root).toHaveAttribute('data-fly-browser-mounted', 'false');
  await expect(root).toHaveAttribute('data-fly-canvas-connected', 'false');
  await expect(page.locator('[data-fly-browser-overlay]')).toHaveCount(0);

  await dispatchCanvasMessage(page, { type: 'ready' }, { sequence: 2 });
  await expect(root).toHaveAttribute('data-fly-canvas-connected', 'false');
});

test('stale save conflict preserves optimistic state and refreshed retry advances it', async ({
  page
}) => {
  await mountAdapter(page);
  await installMockFetch(page, [
    {
      status: 409,
      body: {
        status: 409,
        error: 'Page Builder revision conflict',
        code: 'REVISION_CONFLICT'
      }
    },
    {
      status: 200,
      body: {
        result: {
          revision_id: 'rev-3',
          project_hash: 'hash-3'
        },
        reload: false,
        draft_token: 'draft-3',
        draft_generation: 3
      }
    }
  ]);

  const conflict = await emitBrowserIntent(page, 'save');
  expect(conflict).toMatchObject({
    status: 409,
    code: 'REVISION_CONFLICT'
  });

  let state = await page.evaluate(() => {
    const root = document.querySelector('#fly-root');
    const adapter = (
      globalThis as typeof globalThis & {
        FlyBrowser?: { mount?: (root: Element) => { draftSession?: unknown } | null };
      }
    ).FlyBrowser?.mount?.(root as Element);
    return {
      revision: root?.getAttribute('data-fly-revision'),
      projectHash: root?.getAttribute('data-fly-project-hash'),
      draftSession: adapter?.draftSession ?? null,
      requests: (
        globalThis as typeof globalThis & { __flyRequests?: unknown[] }
      ).__flyRequests,
      events: (
        globalThis as typeof globalThis & { __flyEvents?: unknown[] }
      ).__flyEvents
    };
  });

  expect(state.revision).toBe('rev-1');
  expect(state.projectHash).toBe('hash-1');
  expect(state.draftSession).toBeNull();
  expect(state.requests).toHaveLength(1);
  expect(state.requests?.[0]).toMatchObject({
    input: '/fly-intent',
    method: 'POST',
    body: {
      protocol: 'fly_iframe',
      instance_id: 'canvas-a',
      intent: 'save',
      page_id: 'home',
      revision: 'rev-1',
      project_hash: 'hash-1',
      draft_token: null,
      draft_generation: null
    }
  });
  expect(state.events).toContainEqual({
    type: 'fly:browser-intent-response',
    ok: false,
    status: 409,
    code: 'REVISION_CONFLICT',
    intent: 'save',
    revision: 'rev-1',
    projectHash: 'hash-1'
  });
  expect(state.events).not.toContainEqual(
    expect.objectContaining({ type: 'fly:browser-error' })
  );

  await page.evaluate(() => {
    const root = document.querySelector('#fly-root');
    if (!(root instanceof HTMLElement)) throw new Error('Fly root unavailable');
    root.dataset.flyRevision = 'rev-2';
    root.dataset.flyProjectHash = 'hash-2';
  });

  const success = await emitBrowserIntent(page, 'save');
  expect(success).toMatchObject({
    result: {
      revision_id: 'rev-3',
      project_hash: 'hash-3'
    },
    draft_token: 'draft-3',
    draft_generation: 3
  });

  state = await page.evaluate(() => {
    const root = document.querySelector('#fly-root');
    const adapter = (
      globalThis as typeof globalThis & {
        FlyBrowser?: {
          mount?: (
            root: Element
          ) => { draftSession?: { token?: string; generation?: number } } | null;
        };
      }
    ).FlyBrowser?.mount?.(root as Element);
    return {
      revision: root?.getAttribute('data-fly-revision'),
      projectHash: root?.getAttribute('data-fly-project-hash'),
      draftSession: adapter?.draftSession ?? null,
      requests: (
        globalThis as typeof globalThis & { __flyRequests?: unknown[] }
      ).__flyRequests
    };
  });

  expect(state.requests).toHaveLength(2);
  expect(state.requests?.[1]).toMatchObject({
    body: {
      revision: 'rev-2',
      project_hash: 'hash-2'
    }
  });
  expect(state.revision).toBe('rev-3');
  expect(state.projectHash).toBe('hash-3');
  expect(state.draftSession).toEqual({ token: 'draft-3', generation: 3 });
});
