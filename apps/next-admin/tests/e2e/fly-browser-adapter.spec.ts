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
