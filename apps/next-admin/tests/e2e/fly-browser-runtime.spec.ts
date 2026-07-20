import { expect, Page, test } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const adapterPath = resolve(
  process.cwd(),
  '../../crates/fly-browser/assets/fly-browser.js'
);
const canvasRuntimePath = resolve(
  process.cwd(),
  '../../crates/rustok-page-builder/admin/src/editor/canvas_runtime.js'
);

type RuntimeEvent = {
  type: string;
  sequence?: number;
  messageType?: string;
  message?: Record<string, unknown>;
};

type RuntimeMountOptions = {
  maxGeometryComponents?: number;
};

async function mountRuntime(
  page: Page,
  canvasBody: string,
  options: RuntimeMountOptions = {}
) {
  const [adapterSource, runtimeTemplate] = await Promise.all([
    readFile(adapterPath, 'utf8'),
    readFile(canvasRuntimePath, 'utf8')
  ]);
  const runtimeSource = runtimeTemplate
    .replaceAll('__FLY_PROTOCOL__', JSON.stringify('fly_iframe'))
    .replaceAll('__FLY_INSTANCE__', JSON.stringify('canvas-a'))
    .replaceAll(
      '__FLY_MAX_GEOMETRY_COMPONENTS__',
      String(options.maxGeometryComponents ?? 4096)
    );

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
      <button
        id="begin-drag"
        type="button"
        data-fly-block-id="hero-block"
        data-fly-action="begin-block-drag"
      >Begin hero drag</button>
      <div id="frame-host" style="position:relative;width:420px;height:260px">
        <iframe
          id="canvas-a-frame"
          data-fly-iframe-canvas
          title="Fly test canvas"
          style="display:block;width:420px;height:260px;border:0"
        ></iframe>
      </div>
    </div>
  `);

  await page.evaluate(
    async ({ adapterSource, runtimeSource, canvasBody }) => {
      const root = document.querySelector('#fly-root');
      const iframe = document.querySelector('#canvas-a-frame');
      if (!(root instanceof HTMLElement)) throw new Error('Fly root unavailable');
      if (!(iframe instanceof HTMLIFrameElement)) {
        throw new Error('Fly iframe unavailable');
      }

      const events: RuntimeEvent[] = [];
      root.addEventListener('fly:canvas-message', (event) => {
        const detail = (event as CustomEvent).detail;
        events.push({
          type: event.type,
          sequence: detail?.sequence,
          messageType: detail?.message?.type,
          message: detail?.message
        });
      });
      root.addEventListener('fly:browser-intent-response', (event) => {
        events.push({ type: event.type });
      });
      (
        globalThis as typeof globalThis & {
          __flyRuntimeEvents?: RuntimeEvent[];
        }
      ).__flyRuntimeEvents = events;

      const adapterUrl = URL.createObjectURL(
        new Blob([adapterSource], { type: 'text/javascript' })
      );
      try {
        await import(adapterUrl);
      } finally {
        URL.revokeObjectURL(adapterUrl);
      }

      const escapedRuntime = runtimeSource.replaceAll('</script', '<\\/script');
      iframe.srcdoc = `<!doctype html><html><head><meta charset="utf-8"><style>html,body{margin:0}*{box-sizing:border-box}</style></head><body>${canvasBody}<script>${escapedRuntime}</script></body></html>`;
    },
    { adapterSource, runtimeSource, canvasBody }
  );

  await expect(page.locator('#fly-root')).toHaveAttribute(
    'data-fly-browser-mounted',
    'true'
  );
  await expect(page.locator('#fly-root')).toHaveAttribute(
    'data-fly-canvas-connected',
    'true'
  );
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (
            globalThis as typeof globalThis & {
              __flyRuntimeEvents?: RuntimeEvent[];
            }
          ).__flyRuntimeEvents?.filter(
            (event) => event.messageType === 'geometry_snapshot'
          ).length ?? 0
      )
    )
    .toBeGreaterThan(0);
}

async function geometrySnapshotCount(page: Page) {
  return page.evaluate(
    () =>
      (
        globalThis as typeof globalThis & {
          __flyRuntimeEvents?: RuntimeEvent[];
        }
      ).__flyRuntimeEvents?.filter(
        (event) => event.messageType === 'geometry_snapshot'
      ).length ?? 0
  );
}

async function installDeferredFetch(page: Page) {
  await page.evaluate(() => {
    const requests: unknown[] = [];
    let resolvePending: (() => void) | null = null;
    const scope = globalThis as typeof globalThis & {
      __flyRequests?: unknown[];
      __resolveFlyFetch?: () => void;
    };
    scope.__flyRequests = requests;
    scope.__resolveFlyFetch = () => {
      const resolve = resolvePending;
      resolvePending = null;
      resolve?.();
    };
    globalThis.fetch = async (input, init = {}) => {
      const body =
        typeof init.body === 'string' ? JSON.parse(init.body) : init.body ?? null;
      requests.push({
        input: String(input),
        method: init.method ?? 'GET',
        body
      });
      return new Promise<Response>((resolve) => {
        resolvePending = () =>
          resolve(
            new Response(
              JSON.stringify({
                result: { revision_id: 'rev-2', project_hash: 'hash-2' }
              }),
              {
                status: 200,
                headers: { 'content-type': 'application/json' }
              }
            )
          );
      });
    };
  });
}

test('nested scrolling refreshes component geometry and parent overlays', async ({
  page
}) => {
  await mountRuntime(
    page,
    `
      <div id="nested-scroll" style="height:120px;overflow:auto">
        <div style="height:520px;padding-top:180px">
          <section
            data-fly-component-id="nested"
            data-fly-index="0"
            style="height:40px"
          >Nested component</section>
        </div>
      </div>
    `
  );

  await page.evaluate(() => {
    const root = document.querySelector('#fly-root');
    if (!(root instanceof HTMLElement)) throw new Error('Fly root unavailable');
    root.dispatchEvent(
      new CustomEvent('fly:select', {
        bubbles: true,
        detail: { componentId: 'nested' }
      })
    );
  });

  const overlay = page.locator('[data-fly-browser-overlay="selected"]');
  await expect(overlay).toHaveCSS('display', 'block');
  const initialTop = await overlay.evaluate((element) =>
    Number.parseFloat((element as HTMLElement).style.top)
  );
  const initialSnapshots = await geometrySnapshotCount(page);

  await page
    .frameLocator('#canvas-a-frame')
    .locator('#nested-scroll')
    .evaluate((element) => {
      (element as HTMLElement).scrollTop = 96;
    });

  await expect
    .poll(() => geometrySnapshotCount(page))
    .toBeGreaterThan(initialSnapshots);
  await expect
    .poll(() =>
      overlay.evaluate((element) =>
        Number.parseFloat((element as HTMLElement).style.top)
      )
    )
    .toBeLessThan(initialTop - 70);
});

test('keyboard-started DnD suppresses late drag samples and duplicate drops', async ({
  page
}) => {
  await mountRuntime(
    page,
    `
      <section
        id="drop-target"
        data-fly-component-id="drop-target"
        data-fly-index="0"
        style="margin:20px;width:240px;height:120px"
      >Drop target</section>
    `
  );
  await installDeferredFetch(page);

  const root = page.locator('#fly-root');
  const trigger = page.getByRole('button', { name: 'Begin hero drag' });
  await trigger.focus();
  await page.keyboard.press('Enter');
  await expect(trigger).toBeFocused();
  await expect(root).toHaveAttribute('data-fly-dragging', 'block');

  const startingSequence = await page.evaluate(() =>
    Math.max(
      0,
      ...((
        globalThis as typeof globalThis & {
          __flyRuntimeEvents?: RuntimeEvent[];
        }
      ).__flyRuntimeEvents ?? []).map((event) => event.sequence ?? 0)
    )
  );

  const frame = page.frameLocator('#canvas-a-frame');
  await frame.locator('#drop-target').evaluate((element) => {
    const rect = element.getBoundingClientRect();
    const common = {
      bubbles: true,
      clientX: rect.left + rect.width / 2,
      clientY: rect.top + rect.height / 2,
      pointerId: 1,
      pointerType: 'mouse',
      isPrimary: true
    };
    element.dispatchEvent(new PointerEvent('pointermove', { ...common, buttons: 1 }));
    element.dispatchEvent(new PointerEvent('pointerup', { ...common, buttons: 0 }));
    element.dispatchEvent(new PointerEvent('pointerup', { ...common, buttons: 0 }));
  });
  await frame.locator('body').evaluate(
    () =>
      new Promise<void>((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
      )
  );

  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (
            globalThis as typeof globalThis & { __flyRequests?: unknown[] }
          ).__flyRequests?.length ?? 0
      )
    )
    .toBe(1);

  const state = await page.evaluate((startingSequence) => {
    const scope = globalThis as typeof globalThis & {
      __flyRuntimeEvents?: RuntimeEvent[];
      __flyRequests?: unknown[];
    };
    const messages = (scope.__flyRuntimeEvents ?? []).filter(
      (event) =>
        event.type === 'fly:canvas-message' &&
        (event.sequence ?? 0) > startingSequence
    );
    return {
      messageTypes: messages.map((event) => event.messageType),
      requests: scope.__flyRequests ?? [],
      dragging: document
        .querySelector('#fly-root')
        ?.getAttribute('data-fly-dragging'),
      overlaysHidden: Array.from(
        document.querySelectorAll('[data-fly-browser-overlay]')
      ).every((overlay) => overlay.getAttribute('aria-hidden') === 'true')
    };
  }, startingSequence);

  expect(state.messageTypes.filter((type) => type === 'drop_requested')).toHaveLength(2);
  expect(state.messageTypes).not.toContain('drag_moved');
  expect(state.requests).toHaveLength(1);
  expect(state.requests[0]).toMatchObject({
    input: '/fly-intent',
    method: 'POST',
    body: {
      intent: 'drop',
      payload: {
        source: { kind: 'block', block_id: 'hero-block' },
        target_component_id: 'drop-target',
        position: 'inside'
      }
    }
  });
  expect(state.dragging).toBeNull();
  expect(state.overlaysHidden).toBe(true);
  await expect(
    page.locator('[data-fly-browser-overlay="insertion"]')
  ).toHaveCSS('display', 'none');

  await page.evaluate(() => {
    (
      globalThis as typeof globalThis & { __resolveFlyFetch?: () => void }
    ).__resolveFlyFetch?.();
  });
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (
            globalThis as typeof globalThis & {
              __flyRuntimeEvents?: RuntimeEvent[];
            }
          ).__flyRuntimeEvents?.filter(
            (event) => event.type === 'fly:browser-intent-response'
          ).length ?? 0
      )
    )
    .toBe(1);
});

test('geometry snapshots fail closed when the canvas resource limit is exceeded', async ({
  page
}) => {
  await mountRuntime(
    page,
    `
      <div data-fly-component-id="one"></div>
      <div data-fly-component-id="two"></div>
      <div data-fly-component-id="three"></div>
    `,
    { maxGeometryComponents: 2 }
  );

  const limited = await page.evaluate(() =>
    (
      globalThis as typeof globalThis & {
        __flyRuntimeEvents?: RuntimeEvent[];
      }
    ).__flyRuntimeEvents?.find(
      (event) =>
        event.messageType === 'geometry_snapshot' &&
        Boolean(event.message?.resource_limit)
    )
  );

  expect(limited?.message).toMatchObject({
    components: [],
    resource_limit: {
      kind: 'geometry_components',
      limit: 2,
      observed: 3
    }
  });

  await page.evaluate(() => {
    const root = document.querySelector('#fly-root');
    if (!(root instanceof HTMLElement)) throw new Error('Fly root unavailable');
    root.dispatchEvent(
      new CustomEvent('fly:select', {
        bubbles: true,
        detail: { componentId: 'one' }
      })
    );
  });
  await expect(
    page.locator('[data-fly-browser-overlay="selected"]')
  ).toHaveCSS('display', 'none');
});
