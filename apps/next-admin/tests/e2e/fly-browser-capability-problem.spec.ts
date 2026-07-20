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

type BrowserProblem = {
  status?: number;
  code?: string;
  error?: string;
  intent?: string | null;
  capability?: string | null;
  required?: string[];
  missing?: string[];
  instanceId?: string;
  pageId?: string | null;
};

type BrowserRequest = {
  revision?: string | null;
  project_hash?: string | null;
  draft_token?: string | null;
  draft_generation?: number | null;
  intent?: string;
};

async function mountProblemContract(page: Page) {
  const [adapterSource, hardeningSource] = await Promise.all([
    readFile(adapterPath, 'utf8'),
    readFile(hardeningPath, 'utf8')
  ]);

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
      <div style="position:relative">
        <iframe
          id="canvas-a-frame"
          data-fly-iframe-canvas
          title="Fly capability problem canvas"
        ></iframe>
      </div>
    </div>
  `);

  await page.evaluate(
    async ({ adapterSource, hardeningSource }) => {
      const root = document.querySelector('#fly-root');
      if (!(root instanceof HTMLElement)) throw new Error('Fly root unavailable');

      const scope = globalThis as typeof globalThis & {
        __FLY_BROWSER_CONFIG__?: Record<string, unknown>;
        __flyBrowserProblems?: BrowserProblem[];
        __flyBrowserResponses?: unknown[];
        __flyFetchMode?: 'denied' | 'success';
        __flyRequests?: BrowserRequest[];
      };
      scope.__FLY_BROWSER_CONFIG__ = {
        autoMount: true,
        intentEndpoint: '/fly-intent'
      };
      scope.__flyBrowserProblems = [];
      scope.__flyBrowserResponses = [];
      scope.__flyFetchMode = 'denied';
      scope.__flyRequests = [];
      sessionStorage.setItem(
        'fly:ssr-draft:home',
        JSON.stringify({ token: 'draft-1', generation: 1 })
      );

      root.addEventListener('fly:browser-problem', (event) => {
        scope.__flyBrowserProblems?.push(
          (event as CustomEvent<BrowserProblem>).detail
        );
      });
      root.addEventListener('fly:browser-intent-response', (event) => {
        scope.__flyBrowserResponses?.push((event as CustomEvent).detail);
      });

      globalThis.fetch = async (_input, init = {}) => {
        const request =
          typeof init.body === 'string'
            ? (JSON.parse(init.body) as BrowserRequest)
            : {};
        scope.__flyRequests?.push(request);
        if (scope.__flyFetchMode === 'denied') {
          return new Response(
            JSON.stringify({
              status: 403,
              error: 'browser intent `save` requires editor capability `publish`',
              code: 'FLY_CAPABILITY_DENIED',
              intent: 'save',
              capability: 'publish',
              required: ['publish'],
              missing: ['publish']
            }),
            {
              status: 403,
              headers: { 'content-type': 'application/json' }
            }
          );
        }
        return new Response(
          JSON.stringify({
            result: { revision_id: 'rev-2', project_hash: 'hash-2' },
            draft_token: 'draft-2',
            draft_generation: 2
          }),
          {
            status: 200,
            headers: { 'content-type': 'application/json' }
          }
        );
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
    },
    { adapterSource, hardeningSource }
  );

  await expect(page.locator('#fly-root')).toHaveAttribute(
    'data-fly-browser-mounted',
    'true'
  );
}

async function emitSave(page: Page) {
  await page.evaluate(async () => {
    const scope = globalThis as typeof globalThis & {
      __FLY_BROWSER_CONFIG__?: Record<string, unknown>;
      FlyBrowser?: {
        mountAll?: (options?: Record<string, unknown>) => Array<{
          emitIntent: (
            intent: string,
            payload: Record<string, unknown>
          ) => Promise<unknown>;
        }>;
      };
    };
    const adapter = scope.FlyBrowser?.mountAll?.(
      scope.__FLY_BROWSER_CONFIG__ ?? {}
    )[0];
    if (!adapter) throw new Error('Fly adapter unavailable');
    await adapter.emitIntent('save', { project: { pages: [] } });
  });
}

test('capability denial is typed, accessible and cleared only by success', async ({
  page
}) => {
  await mountProblemContract(page);
  await emitSave(page);

  const root = page.locator('#fly-root');
  await expect(root).toHaveAttribute(
    'data-fly-browser-problem',
    'FLY_CAPABILITY_DENIED'
  );
  await expect(root).toHaveAttribute('data-fly-revision', 'rev-1');
  await expect(root).toHaveAttribute('data-fly-project-hash', 'hash-1');

  const alert = page.locator('[data-fly-browser-status="problem"]');
  await expect(alert).toHaveAttribute('role', 'alert');
  await expect(alert).toHaveAttribute('aria-live', 'assertive');
  await expect(alert).toHaveAttribute('aria-atomic', 'true');
  await expect(alert).toHaveText(
    'browser intent `save` requires editor capability `publish`'
  );

  let state = await page.evaluate(() => {
    const scope = globalThis as typeof globalThis & {
      __flyBrowserProblems?: BrowserProblem[];
      __flyRequests?: BrowserRequest[];
    };
    return {
      problems: scope.__flyBrowserProblems ?? [],
      requests: scope.__flyRequests ?? [],
      draft: JSON.parse(sessionStorage.getItem('fly:ssr-draft:home') ?? 'null')
    };
  });
  expect(state.problems).toEqual([
    {
      status: 403,
      code: 'FLY_CAPABILITY_DENIED',
      error: 'browser intent `save` requires editor capability `publish`',
      intent: 'save',
      capability: 'publish',
      required: ['publish'],
      missing: ['publish'],
      instanceId: 'canvas-a',
      pageId: 'home'
    }
  ]);
  expect(state.requests).toHaveLength(1);
  expect(state.requests[0]).toMatchObject({
    intent: 'save',
    revision: 'rev-1',
    project_hash: 'hash-1',
    draft_token: 'draft-1',
    draft_generation: 1
  });
  expect(state.draft).toEqual({ token: 'draft-1', generation: 1 });

  await page.evaluate(() => {
    (
      globalThis as typeof globalThis & {
        __flyFetchMode?: 'denied' | 'success';
      }
    ).__flyFetchMode = 'success';
  });
  await emitSave(page);

  await expect(root).not.toHaveAttribute('data-fly-browser-problem');
  await expect(alert).toHaveCount(0);
  await expect(root).toHaveAttribute('data-fly-revision', 'rev-2');
  await expect(root).toHaveAttribute('data-fly-project-hash', 'hash-2');

  state = await page.evaluate(() => {
    const scope = globalThis as typeof globalThis & {
      __flyBrowserProblems?: BrowserProblem[];
      __flyBrowserResponses?: unknown[];
      __flyRequests?: BrowserRequest[];
    };
    return {
      problems: scope.__flyBrowserProblems ?? [],
      responses: scope.__flyBrowserResponses ?? [],
      requests: scope.__flyRequests ?? [],
      draft: JSON.parse(sessionStorage.getItem('fly:ssr-draft:home') ?? 'null')
    };
  });
  expect(state.problems).toHaveLength(1);
  expect(state.responses).toHaveLength(2);
  expect(state.requests).toHaveLength(2);
  expect(state.requests[1]).toMatchObject({
    intent: 'save',
    revision: 'rev-1',
    project_hash: 'hash-1',
    draft_token: 'draft-1',
    draft_generation: 1
  });
  expect(state.draft).toEqual({ token: 'draft-2', generation: 2 });

  await page.evaluate(() => {
    (
      globalThis as typeof globalThis & {
        FlyBrowser?: { unmountAll?: () => void };
      }
    ).FlyBrowser?.unmountAll?.();
  });
  await expect(root).toHaveAttribute('data-fly-browser-mounted', 'false');
  await expect(page.locator('[data-fly-browser-status]')).toHaveCount(0);
});
