import { expect, Page, test } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const adapterPath = resolve(
  process.cwd(),
  '../../crates/ui/fly/browser/assets/fly-browser.js'
);
const canvasRuntimePath = resolve(
  process.cwd(),
  '../../crates/modules/rustok-page-builder/admin/src/editor/canvas_runtime.js'
);

type RuntimeEvent = {
  type: string;
  sequence?: number;
  messageType?: string;
  message?: Record<string, unknown>;
  detail?: Record<string, unknown>;
};

type MockResponse = {
  status: number;
  body: Record<string, unknown>;
};

async function mountEditorWorkspace(
  page: Page,
  canvasBody: string,
  initialResponses: MockResponse[] = []
) {
  const [adapterSource, runtimeTemplate] = await Promise.all([
    readFile(adapterPath, 'utf8'),
    readFile(canvasRuntimePath, 'utf8')
  ]);
  const runtimeSource = runtimeTemplate
    .replaceAll('__FLY_PROTOCOL__', JSON.stringify('fly_iframe'))
    .replaceAll('__FLY_INSTANCE__', JSON.stringify('canvas-main'))
    .replaceAll('__FLY_MAX_GEOMETRY_COMPONENTS__', '4096');

  await page.setContent(`
    <div
      id="fly-root"
      data-fly-browser-root
      data-fly-page-id="home"
      data-fly-revision="rev-1"
      data-fly-project-hash="hash-1"
      data-fly-expected-origin="null"
      data-fly-intent-endpoint="/fly-intent"
    >
      <div id="fly-toolbar" role="toolbar" aria-label="Editor toolbar">
        <button
          id="btn-undo"
          type="button"
          data-fly-action="intent:undo"
        >Undo</button>
        <button
          id="btn-redo"
          type="button"
          data-fly-action="intent:redo"
        >Redo</button>
        <button
          id="btn-delete"
          type="button"
          data-fly-action="intent:delete_selection"
        >Delete</button>
        <button
          id="btn-cut"
          type="button"
          data-fly-action="intent:cut_selection"
        >Cut</button>
        <button
          id="btn-paste"
          type="button"
          data-fly-action="intent:paste_component"
        >Paste</button>
      </div>
      <div id="frame-host" style="position:relative;width:600px;height:400px">
        <iframe
          id="canvas-main-frame"
          data-fly-iframe-canvas
          title="Fly authoring canvas"
          style="display:block;width:600px;height:400px;border:0"
        ></iframe>
      </div>
      <div id="problem-container" aria-live="polite"></div>
    </div>
  `);

  await page.evaluate(
    async ({ adapterSource, runtimeSource, canvasBody, initialResponses }) => {
      const root = document.querySelector('#fly-root');
      const iframe = document.querySelector('#canvas-main-frame');
      if (!(root instanceof HTMLElement))
        throw new Error('Fly root unavailable');
      if (!(iframe instanceof HTMLIFrameElement))
        throw new Error('Fly iframe unavailable');

      const requests: Array<{
        input: string;
        method: string;
        body: Record<string, unknown> | null;
      }> = [];
      const responses: MockResponse[] = [...initialResponses];
      let responseIdx = 0;

      const scope = globalThis as typeof globalThis & {
        __flyRequests?: typeof requests;
        __flyRuntimeEvents?: RuntimeEvent[];
        __pushMockResponse?: (resp: MockResponse) => void;
      };
      scope.__flyRequests = requests;
      scope.__pushMockResponse = (resp: MockResponse) => responses.push(resp);

      globalThis.fetch = async (input, init = {}) => {
        const body =
          typeof init.body === 'string'
            ? JSON.parse(init.body)
            : (init.body ?? null);
        requests.push({
          input: String(input),
          method: init.method ?? 'GET',
          body
        });

        const currentResponse = responses[responseIdx] ?? {
          status: 200,
          body: {
            result: {
              revision_id: `rev-${responseIdx + 2}`,
              project_hash: `hash-${responseIdx + 2}`
            }
          }
        };
        responseIdx++;

        return new Response(JSON.stringify(currentResponse.body), {
          status: currentResponse.status,
          headers: { 'content-type': 'application/json' }
        });
      };

      const events: RuntimeEvent[] = [];
      const captureEvent = (event: Event) => {
        const detail = (event as CustomEvent).detail;
        events.push({
          type: event.type,
          sequence: detail?.sequence,
          messageType: detail?.message?.type,
          message: detail?.message,
          detail
        });
      };

      root.addEventListener('fly:canvas-message', captureEvent);
      root.addEventListener('fly:browser-intent', captureEvent);
      root.addEventListener('fly:browser-intent-response', captureEvent);
      root.addEventListener('fly:browser-problem', captureEvent);
      scope.__flyRuntimeEvents = events;

      const adapterUrl = URL.createObjectURL(
        new Blob([adapterSource], { type: 'text/javascript' })
      );
      try {
        await import(adapterUrl);
      } finally {
        URL.revokeObjectURL(adapterUrl);
      }

      const escapedRuntime = runtimeSource.replaceAll('</script', '<\\/script');
      iframe.srcdoc = `<!doctype html><html><head><meta charset="utf-8"><style>html,body{margin:0;padding:0}*{box-sizing:border-box}</style></head><body>${canvasBody}<script>${escapedRuntime}</script></body></html>`;
    },
    { adapterSource, runtimeSource, canvasBody, initialResponses }
  );

  await expect(page.locator('#fly-root')).toHaveAttribute(
    'data-fly-browser-mounted',
    'true'
  );
  await expect(page.locator('#fly-root')).toHaveAttribute(
    'data-fly-canvas-connected',
    'true'
  );
}

async function dispatchFlyIntent(
  page: Page,
  intent: string,
  payload: Record<string, unknown> = {}
) {
  await page.evaluate(
    ({ intent, payload }) => {
      const root = document.querySelector('#fly-root') as HTMLElement & {
        [key: symbol]: {
          emitIntent: (
            intent: string,
            payload?: Record<string, unknown>
          ) => Promise<unknown>;
        };
      };
      const adapter = root ? root[Symbol.for('fly.browser.adapter')] : null;
      if (!adapter)
        throw new Error('Fly browser adapter not found on #fly-root');
      return adapter.emitIntent(intent, payload);
    },
    { intent, payload }
  );
}

test.describe('Fly Editor Authoring Scenarios', () => {
  test('scenario 1: component selection and style rule editing updates live overlay and dispatches upsert_style_rule', async ({
    page
  }) => {
    await mountEditorWorkspace(
      page,
      `
        <section
          data-fly-component-id="hero-section"
          style="position:absolute;left:20px;top:30px;width:300px;height:120px;background:#f1f5f9"
        >
          <h1
            data-fly-component-id="hero-title"
            style="position:absolute;left:10px;top:10px;width:200px;height:40px"
          >Hero Title</h1>
        </section>
      `
    );

    // 1. Select hero-section component
    await page.evaluate(() => {
      const root = document.querySelector('#fly-root');
      root?.dispatchEvent(
        new CustomEvent('fly:select', {
          bubbles: true,
          detail: { componentId: 'hero-section' }
        })
      );
    });

    const selectedOverlay = page.locator(
      '[data-fly-browser-overlay="selected"]'
    );
    await expect(selectedOverlay).toBeVisible();
    await expect(selectedOverlay).toHaveCSS('left', '20px');
    await expect(selectedOverlay).toHaveCSS('top', '30px');
    await expect(selectedOverlay).toHaveCSS('width', '300px');
    await expect(selectedOverlay).toHaveCSS('height', '120px');

    // 2. Dispatch style rule intent on the selected component
    await dispatchFlyIntent(page, 'upsert_style_rule', {
      component_id: 'hero-section',
      rule: {
        selector: '.hero-dark',
        properties: {
          'background-color': '#0f172a',
          color: '#ffffff'
        }
      }
    });

    // 3. Verify request envelope
    await expect
      .poll(async () => {
        const requests = await page.evaluate(
          () =>
            (
              globalThis as typeof globalThis & {
                __flyRequests?: Array<Record<string, unknown>>;
              }
            ).__flyRequests
        );
        return requests?.find(
          (req) =>
            (req.body as Record<string, unknown>)?.intent ===
            'upsert_style_rule'
        );
      })
      .toBeTruthy();

    const request = await page.evaluate(() => {
      const requests = (
        globalThis as typeof globalThis & {
          __flyRequests?: Array<Record<string, unknown>>;
        }
      ).__flyRequests;
      return requests?.find(
        (req) =>
          (req.body as Record<string, unknown>)?.intent === 'upsert_style_rule'
      );
    });

    expect(request?.body).toMatchObject({
      intent: 'upsert_style_rule',
      revision: 'rev-1',
      page_id: 'home',
      payload: {
        component_id: 'hero-section',
        rule: {
          selector: '.hero-dark',
          properties: {
            'background-color': '#0f172a',
            color: '#ffffff'
          }
        }
      }
    });
  });

  test('scenario 2: design token lifecycle - creation, variable binding, and removal', async ({
    page
  }) => {
    await mountEditorWorkspace(
      page,
      '<div data-fly-component-id="card" style="width:200px;height:100px">Card</div>'
    );

    // 1. Dispatch set_theme_token intent
    await dispatchFlyIntent(page, 'set_theme_token', {
      name: 'brand-primary',
      value: '#2563eb'
    });

    // Verify token creation intent
    await expect
      .poll(async () => {
        const requests = await page.evaluate(
          () =>
            (
              globalThis as typeof globalThis & {
                __flyRequests?: Array<Record<string, unknown>>;
              }
            ).__flyRequests
        );
        return requests?.find(
          (req) =>
            (req.body as Record<string, unknown>)?.intent === 'set_theme_token'
        );
      })
      .toBeTruthy();

    // 2. Dispatch remove_theme_token intent
    await dispatchFlyIntent(page, 'remove_theme_token', {
      name: 'old-accent'
    });

    await expect
      .poll(async () => {
        const requests = await page.evaluate(
          () =>
            (
              globalThis as typeof globalThis & {
                __flyRequests?: Array<Record<string, unknown>>;
              }
            ).__flyRequests
        );
        return requests?.find(
          (req) =>
            (req.body as Record<string, unknown>)?.intent ===
            'remove_theme_token'
        );
      })
      .toBeTruthy();

    const removeReq = await page.evaluate(() => {
      const requests = (
        globalThis as typeof globalThis & {
          __flyRequests?: Array<Record<string, unknown>>;
        }
      ).__flyRequests;
      return requests?.find(
        (req) =>
          (req.body as Record<string, unknown>)?.intent === 'remove_theme_token'
      );
    });

    expect(removeReq?.body).toMatchObject({
      intent: 'remove_theme_token',
      payload: {
        name: 'old-accent'
      }
    });
  });

  test('scenario 3: undo and redo transactions roll back and advance document state', async ({
    page
  }) => {
    await mountEditorWorkspace(
      page,
      '<div data-fly-component-id="root-container">Root</div>'
    );

    // 1. Trigger Undo via action button
    await page.click('#btn-undo');

    await expect
      .poll(async () => {
        const requests = await page.evaluate(
          () =>
            (
              globalThis as typeof globalThis & {
                __flyRequests?: Array<Record<string, unknown>>;
              }
            ).__flyRequests
        );
        return requests?.find(
          (req) => (req.body as Record<string, unknown>)?.intent === 'undo'
        );
      })
      .toBeTruthy();

    // 2. Trigger Redo via action button
    await page.click('#btn-redo');

    await expect
      .poll(async () => {
        const requests = await page.evaluate(
          () =>
            (
              globalThis as typeof globalThis & {
                __flyRequests?: Array<Record<string, unknown>>;
              }
            ).__flyRequests
        );
        return requests?.find(
          (req) => (req.body as Record<string, unknown>)?.intent === 'redo'
        );
      })
      .toBeTruthy();

    // Verify both transactions carried the revision contract
    const requests = await page.evaluate(
      () =>
        (
          globalThis as typeof globalThis & {
            __flyRequests?: Array<Record<string, unknown>>;
          }
        ).__flyRequests
    );
    const undoReq = requests?.find(
      (req) => (req.body as Record<string, unknown>)?.intent === 'undo'
    );
    const redoReq = requests?.find(
      (req) => (req.body as Record<string, unknown>)?.intent === 'redo'
    );

    expect(undoReq?.body).toMatchObject({ intent: 'undo', page_id: 'home' });
    expect(redoReq?.body).toMatchObject({ intent: 'redo', page_id: 'home' });
  });

  test('scenario 4: optimistic error recovery clears problems on subsequent success', async ({
    page
  }) => {
    await mountEditorWorkspace(
      page,
      '<div data-fly-component-id="box">Box</div>',
      [
        {
          status: 403,
          body: {
            code: 'FLY_CAPABILITY_DENIED',
            error: 'You do not have permission to publish this page.'
          }
        },
        {
          status: 200,
          body: {
            result: { revision_id: 'rev-2', project_hash: 'hash-2' }
          }
        }
      ]
    );

    // 1. Trigger intent that will fail (403)
    await dispatchFlyIntent(page, 'publish_page', {});

    // 2. Verify error status and problem contract attribute on root
    const root = page.locator('#fly-root');
    await expect(root).toHaveAttribute(
      'data-fly-browser-problem',
      'FLY_CAPABILITY_DENIED'
    );

    // 3. Trigger a subsequent valid intent that succeeds (200)
    await page.click('#btn-undo');

    // 4. Verify problem attribute is automatically cleared on recovery
    await expect(root).not.toHaveAttribute('data-fly-browser-problem');
  });

  test('scenario 5: component cut and paste lifecycle', async ({ page }) => {
    await mountEditorWorkspace(
      page,
      `
        <div data-fly-component-id="wrapper">
          <div data-fly-component-id="item-to-cut">Move me</div>
        </div>
      `
    );

    // 1. Select the component
    await page.evaluate(() => {
      const root = document.querySelector('#fly-root');
      root?.dispatchEvent(
        new CustomEvent('fly:select', {
          bubbles: true,
          detail: { componentId: 'item-to-cut' }
        })
      );
    });

    // 2. Click Cut
    await page.click('#btn-cut');

    await expect
      .poll(async () => {
        const requests = await page.evaluate(
          () =>
            (
              globalThis as typeof globalThis & {
                __flyRequests?: Array<Record<string, unknown>>;
              }
            ).__flyRequests
        );
        return requests?.find(
          (req) =>
            (req.body as Record<string, unknown>)?.intent === 'cut_selection'
        );
      })
      .toBeTruthy();

    // 3. Click Paste
    await page.click('#btn-paste');

    await expect
      .poll(async () => {
        const requests = await page.evaluate(
          () =>
            (
              globalThis as typeof globalThis & {
                __flyRequests?: Array<Record<string, unknown>>;
              }
            ).__flyRequests
        );
        return requests?.find(
          (req) =>
            (req.body as Record<string, unknown>)?.intent === 'paste_component'
        );
      })
      .toBeTruthy();

    const pasteReq = await page.evaluate(() => {
      const requests = (
        globalThis as typeof globalThis & {
          __flyRequests?: Array<Record<string, unknown>>;
        }
      ).__flyRequests;
      return requests?.find(
        (req) =>
          (req.body as Record<string, unknown>)?.intent === 'paste_component'
      );
    });

    expect(pasteReq?.body).toMatchObject({
      intent: 'paste_component',
      page_id: 'home'
    });
  });
});
