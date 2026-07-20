import { expect, Page, test } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const adapterPath = resolve(
  process.cwd(),
  '../../crates/fly-browser/assets/fly-browser.js'
);
const hardeningPath = resolve(
  process.cwd(),
  '../../crates/fly-browser/assets/browser_hardening.js'
);

type ResourceLimitEvent = {
  kind?: string;
  limit?: number;
  observed?: number;
  instanceId?: string;
  pageId?: string;
};

async function mountResourceGuard(page: Page) {
  const [adapterSource, hardeningSource] = await Promise.all([
    readFile(adapterPath, 'utf8'),
    readFile(hardeningPath, 'utf8')
  ]);

  await page.setContent(`
    <div
      id="fly-root"
      data-fly-browser-root
      data-fly-page-id="home"
      data-fly-expected-origin="null"
      style="position:relative"
    >
      <div style="position:relative">
        <iframe
          id="canvas-a-frame"
          data-fly-iframe-canvas
          title="Fly resource-limits canvas"
        ></iframe>
      </div>
    </div>
  `);

  await page.evaluate(
    async ({ adapterSource, hardeningSource }) => {
      const root = document.querySelector('#fly-root');
      const iframe = document.querySelector('#canvas-a-frame');
      if (!(root instanceof HTMLElement)) throw new Error('Fly root unavailable');
      if (!(iframe instanceof HTMLIFrameElement)) {
        throw new Error('Fly iframe unavailable');
      }

      const limits: ResourceLimitEvent[] = [];
      root.addEventListener('fly:browser-resource-limit', (event) => {
        limits.push((event as CustomEvent).detail ?? {});
      });
      (
        globalThis as typeof globalThis & {
          __flyResourceLimits?: ResourceLimitEvent[];
          __FLY_BROWSER_CONFIG__?: Record<string, unknown>;
        }
      ).__flyResourceLimits = limits;
      (
        globalThis as typeof globalThis & {
          __FLY_BROWSER_CONFIG__?: Record<string, unknown>;
        }
      ).__FLY_BROWSER_CONFIG__ = {
        maxMessageBytes: 1024,
        maxGeometryComponents: 2,
        resourceLimitMessage: 'Canvas limit reached.'
      };

      const url = URL.createObjectURL(
        new Blob([adapterSource, hardeningSource], {
          type: 'text/javascript'
        })
      );
      try {
        await import(url);
      } finally {
        URL.revokeObjectURL(url);
      }

      iframe.srcdoc = `<!doctype html>
        <html>
          <body>
            <script>
              const send = (sequence, message) => parent.postMessage(JSON.stringify({
                protocol: 'fly_iframe',
                instance_id: 'canvas-a',
                sequence,
                message
              }), '*');
              send(1, { type: 'ready' });
              send(2, {
                type: 'geometry_snapshot',
                components: [
                  { component_id: 'one', rect: { left: 0, top: 0, width: 10, height: 10 } },
                  { component_id: 'two', rect: { left: 10, top: 10, width: 10, height: 10 } },
                  { component_id: 'three', rect: { left: 20, top: 20, width: 10, height: 10 } }
                ]
              });
            <\/script>
          </body>
        </html>`;
    },
    { adapterSource, hardeningSource }
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

test('trusted iframe limits are typed, fail closed and announced accessibly', async ({
  page
}) => {
  await mountResourceGuard(page);

  const root = page.locator('#fly-root');
  await expect(root).toHaveAttribute(
    'data-fly-resource-limited',
    'geometry_components'
  );

  const status = page.locator('[data-fly-browser-status="resource-limit"]');
  await expect(status).toHaveAttribute('role', 'status');
  await expect(status).toHaveAttribute('aria-live', 'polite');
  await expect(status).toHaveAttribute('aria-atomic', 'true');
  await expect(status).toContainText(
    'Canvas limit reached. geometry_components: 3/2.'
  );

  let limits = await page.evaluate(
    () =>
      (
        globalThis as typeof globalThis & {
          __flyResourceLimits?: ResourceLimitEvent[];
        }
      ).__flyResourceLimits ?? []
  );
  expect(limits).toContainEqual({
    kind: 'geometry_components',
    limit: 2,
    observed: 3,
    instanceId: 'canvas-a',
    pageId: 'home'
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

  await page
    .frameLocator('#canvas-a-frame')
    .locator('body')
    .evaluate(() => {
      parent.postMessage('x'.repeat(2048), '*');
    });

  await expect(root).toHaveAttribute(
    'data-fly-resource-limited',
    'message_bytes'
  );
  await expect(status).toContainText('Canvas limit reached. message_bytes: 2048/1024.');

  limits = await page.evaluate(
    () =>
      (
        globalThis as typeof globalThis & {
          __flyResourceLimits?: ResourceLimitEvent[];
        }
      ).__flyResourceLimits ?? []
  );
  expect(limits).toContainEqual({
    kind: 'message_bytes',
    limit: 1024,
    observed: 2048,
    instanceId: 'canvas-a',
    pageId: 'home'
  });

  await page.evaluate(() => {
    (
      globalThis as typeof globalThis & {
        FlyBrowser?: { unmountAll?: () => void };
      }
    ).FlyBrowser?.unmountAll?.();
  });
  await expect(status).toHaveCount(0);
  await expect(root).not.toHaveAttribute('data-fly-resource-limited');
});
