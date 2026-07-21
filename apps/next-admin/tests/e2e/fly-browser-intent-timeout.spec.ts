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

type BrowserRequest = {
  intent?: string;
  revision?: string | null;
  project_hash?: string | null;
  draft_token?: string | null;
  draft_generation?: number | null;
};

type IntentTimeout = {
  code?: string;
  error?: string;
  intent?: string | null;
  timeoutMs?: number;
  requestGeneration?: number;
  current?: boolean;
  instanceId?: string;
  pageId?: string | null;
};

type BrowserError = {
  error?: string;
  requestGeneration?: number;
  current?: boolean;
};

type BrowserResponse = {
  status?: number;
  requestGeneration?: number;
  current?: boolean;
};

type TimeoutScope = typeof globalThis & {
  __FLY_BROWSER_CONFIG__?: Record<string, unknown>;
  __flyAbortedRequests?: number;
  __flyErrors?: BrowserError[];
  __flyFetchMode?: 'hang' | 'success';
  __flyProblems?: Array<{ code?: string; error?: string }>;
  __flyRequests?: BrowserRequest[];
  __flyResponses?: BrowserResponse[];
  __flySavePromises?: Promise<unknown>[];
  __flyTimeouts?: IntentTimeout[];
  FlyBrowser?: {
    mountAll?: (options?: Record<string, unknown>) => Array<{
      emitIntent: (
        intent: string,
        payload: Record<string, unknown>
      ) => Promise<unknown>;
    }>;
    unmountAll?: () => void;
  };
};

async function mountTimeoutContract(page: Page, timeoutMs: number) {
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
          title="Fly intent-timeout canvas"
        ></iframe>
      </div>
    </div>
  `);

  await page.evaluate(
    async ({ adapterSource, hardeningSource, timeoutMs }) => {
      const root = document.querySelector('#fly-root');
      if (!(root instanceof HTMLElement)) throw new Error('Fly root unavailable');

      const scope = globalThis as TimeoutScope;
      scope.__FLY_BROWSER_CONFIG__ = {
        autoMount: true,
        intentEndpoint: '/fly-intent',
        maxPendingIntentRequests: 1,
        intentRequestTimeoutMs: timeoutMs,
        intentRequestTimeoutMessage: 'Editor save timed out'
      };
      scope.__flyAbortedRequests = 0;
      scope.__flyErrors = [];
      scope.__flyFetchMode = 'hang';
      scope.__flyProblems = [];
      scope.__flyRequests = [];
      scope.__flyResponses = [];
      scope.__flySavePromises = [];
      scope.__flyTimeouts = [];
      sessionStorage.setItem(
        'fly:ssr-draft:home',
        JSON.stringify({ token: 'draft-1', generation: 1 })
      );

      root.addEventListener('fly:browser-intent-timeout', (event) => {
        scope.__flyTimeouts?.push(
          (event as CustomEvent<IntentTimeout>).detail
        );
      });
      root.addEventListener('fly:browser-problem', (event) => {
        scope.__flyProblems?.push(
          (event as CustomEvent<{ code?: string; error?: string }>).detail
        );
      });
      root.addEventListener('fly:browser-error', (event) => {
        scope.__flyErrors?.push((event as CustomEvent<BrowserError>).detail);
      });
      root.addEventListener('fly:browser-intent-response', (event) => {
        scope.__flyResponses?.push(
          (event as CustomEvent<BrowserResponse>).detail
        );
      });

      globalThis.fetch = async (_input, init = {}) => {
        const request =
          typeof init.body === 'string'
            ? (JSON.parse(init.body) as BrowserRequest)
            : {};
        const signal = init.signal;
        scope.__flyRequests?.push(request);
        if (scope.__flyFetchMode === 'success') {
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
        }
        return new Promise<Response>((_resolve, reject) => {
          const rejectAborted = () => {
            scope.__flyAbortedRequests = (scope.__flyAbortedRequests ?? 0) + 1;
            reject(new DOMException('Aborted', 'AbortError'));
          };
          if (signal?.aborted) {
            rejectAborted();
            return;
          }
          signal?.addEventListener('abort', rejectAborted, { once: true });
        });
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
    { adapterSource, hardeningSource, timeoutMs }
  );

  await expect(page.locator('#fly-root')).toHaveAttribute(
    'data-fly-browser-mounted',
    'true'
  );
}

async function emitSave(page: Page, saveNumber: number) {
  await page.evaluate((number) => {
    const scope = globalThis as TimeoutScope;
    const adapter = scope.FlyBrowser?.mountAll?.(
      scope.__FLY_BROWSER_CONFIG__ ?? {}
    )[0];
    if (!adapter) throw new Error('Fly adapter unavailable');
    scope.__flySavePromises?.push(
      adapter.emitIntent('save', {
        project: { pages: [] },
        save_number: number
      })
    );
  }, saveNumber);
}

async function awaitSave(page: Page, index: number) {
  await page.evaluate(async (saveIndex) => {
    const promise = (globalThis as TimeoutScope).__flySavePromises?.[saveIndex];
    if (!promise) throw new Error(`Save promise ${saveIndex} unavailable`);
    await promise;
  }, index);
}

async function readState(page: Page) {
  return page.evaluate(() => {
    const scope = globalThis as TimeoutScope;
    const root = document.querySelector('#fly-root');
    return {
      aborted: scope.__flyAbortedRequests ?? 0,
      errors: scope.__flyErrors ?? [],
      mounted: root?.getAttribute('data-fly-browser-mounted'),
      problem: root?.getAttribute('data-fly-browser-problem'),
      projectHash: root?.getAttribute('data-fly-project-hash'),
      problems: scope.__flyProblems ?? [],
      requests: scope.__flyRequests ?? [],
      responses: scope.__flyResponses ?? [],
      revision: root?.getAttribute('data-fly-revision'),
      timeouts: scope.__flyTimeouts ?? [],
      draft: JSON.parse(
        sessionStorage.getItem('fly:ssr-draft:home') ?? 'null'
      )
    };
  });
}

test('hung intent times out, releases its slot and allows a successful retry', async ({
  page
}) => {
  await mountTimeoutContract(page, 40);
  await emitSave(page, 1);
  await awaitSave(page, 0);

  const root = page.locator('#fly-root');
  await expect(root).toHaveAttribute(
    'data-fly-browser-problem',
    'INTENT_REQUEST_TIMEOUT'
  );
  const alert = page.locator('[data-fly-browser-status="problem"]');
  await expect(alert).toHaveAttribute('role', 'alert');
  await expect(alert).toHaveText('Editor save timed out after 40 ms.');

  let state = await readState(page);
  expect(state).toMatchObject({
    aborted: 1,
    problem: 'INTENT_REQUEST_TIMEOUT',
    revision: 'rev-1',
    projectHash: 'hash-1',
    draft: { token: 'draft-1', generation: 1 }
  });
  expect(state.timeouts).toEqual([
    {
      code: 'INTENT_REQUEST_TIMEOUT',
      error: 'Editor save timed out after 40 ms.',
      intent: 'save',
      timeoutMs: 40,
      requestGeneration: 1,
      current: true,
      instanceId: 'canvas-a',
      pageId: 'home'
    }
  ]);
  expect(state.problems).toHaveLength(1);
  expect(state.problems[0]).toMatchObject({
    code: 'INTENT_REQUEST_TIMEOUT',
    error: 'Editor save timed out after 40 ms.'
  });
  expect(state.errors).toHaveLength(1);
  expect(state.errors[0]).toMatchObject({
    requestGeneration: 1,
    current: true
  });
  expect(state.requests).toHaveLength(1);
  expect(state.responses).toEqual([]);

  await page.evaluate(() => {
    (globalThis as TimeoutScope).__flyFetchMode = 'success';
  });
  await emitSave(page, 2);
  await awaitSave(page, 1);

  await expect(root).not.toHaveAttribute('data-fly-browser-problem');
  await expect(alert).toHaveCount(0);
  state = await readState(page);
  expect(state).toMatchObject({
    aborted: 1,
    problem: null,
    revision: 'rev-2',
    projectHash: 'hash-2',
    draft: { token: 'draft-2', generation: 2 }
  });
  expect(state.requests).toHaveLength(2);
  expect(state.timeouts).toHaveLength(1);
  expect(state.responses).toHaveLength(1);
  expect(state.responses[0]).toMatchObject({
    status: 200,
    requestGeneration: 2,
    current: true
  });
});

test('unmount clears the timer and aborts without publishing a timeout', async ({
  page
}) => {
  await mountTimeoutContract(page, 80);
  await emitSave(page, 1);
  await expect
    .poll(() =>
      page.evaluate(
        () => (globalThis as TimeoutScope).__flyRequests?.length ?? 0
      )
    )
    .toBe(1);

  await page.evaluate(() => {
    (globalThis as TimeoutScope).FlyBrowser?.unmountAll?.();
  });
  await awaitSave(page, 0);
  await page.waitForTimeout(120);

  const state = await readState(page);
  expect(state).toMatchObject({
    aborted: 1,
    mounted: 'false',
    problem: null,
    revision: 'rev-1',
    projectHash: 'hash-1',
    draft: { token: 'draft-1', generation: 1 },
    problems: [],
    responses: [],
    timeouts: []
  });
  expect(state.errors).toHaveLength(1);
  expect(state.errors[0]).toMatchObject({
    requestGeneration: 1,
    current: false
  });
  await expect(page.locator('[data-fly-browser-status]')).toHaveCount(0);
});
