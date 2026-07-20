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

type ListenerRegistration = {
  target: string;
  type: string;
};

type ManualMountState = {
  sameAdapter: boolean;
  listenersAfterFirst: number;
  listenersAfterSecond: number;
};

async function loadManualBrowser(page: Page) {
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
          title="Fly manual-mount canvas"
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

      const scope = globalThis as typeof globalThis & {
        __FLY_BROWSER_CONFIG__?: Record<string, unknown>;
        __flyBootstrapResultCount?: number;
        __flyListenerRegistrations?: ListenerRegistration[];
        __flyManualMount?: ManualMountState;
        __flyReadyEvents?: number;
        FlyBrowser?: {
          bootstrap?: (options?: Record<string, unknown>) => unknown[];
          mountAll?: (options?: Record<string, unknown>) => unknown[];
          unmountAll?: () => void;
        };
      };
      const config = {
        autoMount: false,
        maxMessageBytes: 1024,
        maxGeometryComponents: 8
      };
      scope.__FLY_BROWSER_CONFIG__ = config;
      scope.__flyReadyEvents = 0;
      root.addEventListener('fly:browser-ready', () => {
        scope.__flyReadyEvents = (scope.__flyReadyEvents ?? 0) + 1;
      });

      const registrations: ListenerRegistration[] = [];
      const watched = new Map<EventTarget, string>([
        [window, 'window'],
        [document, 'document'],
        [root, 'root'],
        [iframe, 'iframe']
      ]);
      const originalAddEventListener = EventTarget.prototype.addEventListener;
      EventTarget.prototype.addEventListener = function addEventListenerWithProbe(
        type,
        listener,
        options
      ) {
        const target = watched.get(this);
        if (target) registrations.push({ target, type });
        return originalAddEventListener.call(this, type, listener, options);
      };
      scope.__flyListenerRegistrations = registrations;

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

      const bootstrapResult = scope.FlyBrowser?.bootstrap?.(config) ?? [];
      scope.__flyBootstrapResultCount = bootstrapResult.length;
      iframe.srcdoc = `<!doctype html>
        <html>
          <body>
            <script>
              parent.postMessage(JSON.stringify({
                protocol: 'fly_iframe',
                instance_id: 'canvas-a',
                sequence: 1,
                message: { type: 'ready' }
              }), '*');
            <\/script>
          </body>
        </html>`;
    },
    { adapterSource, hardeningSource }
  );

  await expect(page.frameLocator('#canvas-a-frame').locator('body')).toHaveCount(1);
}

test('autoMount false stays inert until an explicit idempotent manual mount', async ({
  page
}) => {
  await loadManualBrowser(page);

  const root = page.locator('#fly-root');
  await expect(root).not.toHaveAttribute('data-fly-browser-mounted');
  await expect(root).not.toHaveAttribute('data-fly-canvas-connected');
  await expect(page.locator('[data-fly-browser-overlay]')).toHaveCount(0);

  const preMount = await page.evaluate(() => {
    const scope = globalThis as typeof globalThis & {
      __flyBootstrapResultCount?: number;
      __flyListenerRegistrations?: ListenerRegistration[];
      __flyReadyEvents?: number;
    };
    return {
      bootstrapResultCount: scope.__flyBootstrapResultCount,
      listeners: scope.__flyListenerRegistrations ?? [],
      readyEvents: scope.__flyReadyEvents ?? 0
    };
  });
  expect(preMount.bootstrapResultCount).toBe(0);
  expect(preMount.listeners).toEqual([]);
  expect(preMount.readyEvents).toBe(0);

  await page.evaluate(() => {
    const scope = globalThis as typeof globalThis & {
      __FLY_BROWSER_CONFIG__?: Record<string, unknown>;
      __flyListenerRegistrations?: ListenerRegistration[];
      __flyManualMount?: ManualMountState;
      FlyBrowser?: {
        mountAll?: (options?: Record<string, unknown>) => unknown[];
      };
    };
    const config = scope.__FLY_BROWSER_CONFIG__ ?? {};
    const first = scope.FlyBrowser?.mountAll?.(config) ?? [];
    const listenersAfterFirst = scope.__flyListenerRegistrations?.length ?? 0;
    const second = scope.FlyBrowser?.mountAll?.(config) ?? [];
    scope.__flyManualMount = {
      sameAdapter: first[0] === second[0],
      listenersAfterFirst,
      listenersAfterSecond: scope.__flyListenerRegistrations?.length ?? 0
    };
  });

  await expect(root).toHaveAttribute('data-fly-browser-mounted', 'true');
  await expect(page.locator('[data-fly-browser-overlay]')).toHaveCount(3);

  const mounted = await page.evaluate(() => {
    const scope = globalThis as typeof globalThis & {
      __flyListenerRegistrations?: ListenerRegistration[];
      __flyManualMount?: ManualMountState;
      __flyReadyEvents?: number;
    };
    return {
      listeners: scope.__flyListenerRegistrations ?? [],
      manualMount: scope.__flyManualMount,
      readyEvents: scope.__flyReadyEvents ?? 0
    };
  });
  expect(mounted.manualMount?.sameAdapter).toBe(true);
  expect(mounted.manualMount?.listenersAfterFirst).toBeGreaterThan(0);
  expect(mounted.manualMount?.listenersAfterSecond).toBe(
    mounted.manualMount?.listenersAfterFirst
  );
  expect(mounted.listeners).toEqual(
    expect.arrayContaining([
      { target: 'window', type: 'message' },
      { target: 'root', type: 'fly:select' },
      { target: 'root', type: 'click' },
      { target: 'iframe', type: 'load' }
    ])
  );
  expect(mounted.readyEvents).toBe(1);

  await page
    .frameLocator('#canvas-a-frame')
    .locator('body')
    .evaluate(() => {
      parent.postMessage(
        JSON.stringify({
          protocol: 'fly_iframe',
          instance_id: 'canvas-a',
          sequence: 2,
          message: { type: 'ready' }
        }),
        '*'
      );
    });
  await expect(root).toHaveAttribute('data-fly-canvas-connected', 'true');

  await page.evaluate(() => {
    (
      globalThis as typeof globalThis & {
        FlyBrowser?: { unmountAll?: () => void };
      }
    ).FlyBrowser?.unmountAll?.();
  });
  await expect(root).toHaveAttribute('data-fly-browser-mounted', 'false');
  await expect(root).toHaveAttribute('data-fly-canvas-connected', 'false');
  await expect(page.locator('[data-fly-browser-overlay]')).toHaveCount(0);

  await page
    .frameLocator('#canvas-a-frame')
    .locator('body')
    .evaluate(() => {
      parent.postMessage(
        JSON.stringify({
          protocol: 'fly_iframe',
          instance_id: 'canvas-a',
          sequence: 3,
          message: { type: 'ready' }
        }),
        '*'
      );
    });
  await page.waitForTimeout(50);
  await expect(root).toHaveAttribute('data-fly-canvas-connected', 'false');

  const finalState = await page.evaluate(() => ({
    mounted: document
      .querySelector('#fly-root')
      ?.getAttribute('data-fly-browser-mounted')
  }));
  expect(finalState.mounted).toBe('false');
});
