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

type PendingFetch = {
  request: BrowserRequest;
  signal?: AbortSignal | null;
  resolve: (response: Response) => void;
};

type RejectedIntent = {
  code?: string;
  error?: string;
  intent?: string | null;
  limit?: number;
  observed?: number;
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

type PendingIntentScope = typeof globalThis & {
  __FLY_BROWSER_CONFIG__?: Record<string, unknown>;
  __flyAbortedRequests?: number;
  __flyErrors?: BrowserError[];
  __flyPendingFetches?: PendingFetch[];
  __flyProblems?: Array<{ code?: string }>;
  __flyRejectedIntents?: RejectedIntent[];
  __flyRequests?: BrowserRequest[];
  __flyResponses?: BrowserResponse[];
  __flySavePromises?: Promise<unknown>[];
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

async function mountPendingIntentContract(page: Page) {
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
          title="Fly pending-intent canvas"
        ></iframe>
      </div>
    </div>
  `);

  await page.evaluate(
    async ({ adapterSource, hardeningSource }) => {
      const root = document.querySelector('#fly-root');
      if (!(root instanceof HTMLElement)) throw new Error('Fly root unavailable');

      const scope = globalThis as PendingIntentScope;
      scope.__FLY_BROWSER_CONFIG__ = {
        autoMount: true,
        intentEndpoint: '/fly-intent',
        maxPendingIntentRequests: 2,
        pendingIntentLimitMessage: 'Too many editor actions.'
      };
      scope.__flyAbortedRequests = 0;
      scope.__flyErrors = [];
      scope.__flyPendingFetches = [];
      scope.__flyProblems = [];
      scope.__flyRejectedIntents = [];
      scope.__flyRequests = [];
      scope.__flyResponses = [];
      scope.__flySavePromises = [];
      sessionStorage.setItem(
        'fly:ssr-draft:home',
        JSON.stringify({ token: 'draft-1', generation: 1 })
      );

      root.addEventListener('fly:browser-intent-rejected', (event) => {
        scope.__flyRejectedIntents?.push(
          (event as CustomEvent<RejectedIntent>).detail
        );
      });
      root.addEventListener('fly:browser-problem', (event) => {
        scope.__flyProblems?.push(
          (event as CustomEvent<{ code?: string }>).detail
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
        return new Promise<Response>((resolveResponse, rejectResponse) => {
          const rejectAborted = () => {
            scope.__flyAbortedRequests = (scope.__flyAbortedRequests ?? 0) + 1;
            rejectResponse(new DOMException('Aborted', 'AbortError'));
          };
          if (signal?.aborted) {
            rejectAborted();
            return;
          }
          signal?.addEventListener('abort', rejectAborted, { once: true });
          scope.__flyPendingFetches?.push({
            request,
            signal,
            resolve: resolveResponse
          });
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
    { adapterSource, hardeningSource }
  );

  await expect(page.locator('#fly-root')).toHaveAttribute(
    'data-fly-browser-mounted',
    'true'
  );
}

async function emitSave(page: Page, saveNumber: number) {
  await page.evaluate((number) => {
    const scope = globalThis as PendingIntentScope;
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
    const promise = (globalThis as PendingIntentScope).__flySavePromises?.[
      saveIndex
    ];
    if (!promise) throw new Error(`Save promise ${saveIndex} unavailable`);
    await promise;
  }, index);
}

async function resolveFetch(
  page: Page,
  index: number,
  revision: string,
  generation: number
) {
  await page.evaluate(
    ({ index, revision, generation }) => {
      const pending = (globalThis as PendingIntentScope).__flyPendingFetches?.[
        index
      ];
      if (!pending) throw new Error(`Pending fetch ${index} unavailable`);
      pending.resolve(
        new Response(
          JSON.stringify({
            result: {
              revision_id: revision,
              project_hash: `hash-${generation}`
            },
            draft_token: `draft-${generation}`,
            draft_generation: generation
          }),
          {
            status: 200,
            headers: { 'content-type': 'application/json' }
          }
        )
      );
    },
    { index, revision, generation }
  );
}

async function readState(page: Page) {
  return page.evaluate(() => {
    const scope = globalThis as PendingIntentScope;
    const root = document.querySelector('#fly-root');
    return {
      aborted: scope.__flyAbortedRequests ?? 0,
      errors: scope.__flyErrors ?? [],
      mounted: root?.getAttribute('data-fly-browser-mounted'),
      problem: root?.getAttribute('data-fly-browser-problem'),
      projectHash: root?.getAttribute('data-fly-project-hash'),
      rejected: scope.__flyRejectedIntents ?? [],
      requests: scope.__flyRequests ?? [],
      responses: scope.__flyResponses ?? [],
      revision: root?.getAttribute('data-fly-revision'),
      draft: JSON.parse(
        sessionStorage.getItem('fly:ssr-draft:home') ?? 'null'
      )
    };
  });
}

test('pending intent limit rejects newest work and releases settled slots', async ({
  page
}) => {
  await mountPendingIntentContract(page);
  await emitSave(page, 1);
  await emitSave(page, 2);
  await emitSave(page, 3);
  await awaitSave(page, 2);

  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (globalThis as PendingIntentScope).__flyPendingFetches?.length ?? 0
      )
    )
    .toBe(2);

  const root = page.locator('#fly-root');
  await expect(root).toHaveAttribute(
    'data-fly-browser-problem',
    'PENDING_INTENT_LIMIT'
  );
  const alert = page.locator('[data-fly-browser-status="problem"]');
  await expect(alert).toHaveAttribute('role', 'alert');
  await expect(alert).toHaveText('Too many editor actions. 3/2.');

  let state = await readState(page);
  expect(state.requests).toHaveLength(2);
  expect(state.rejected).toEqual([
    {
      code: 'PENDING_INTENT_LIMIT',
      error: 'Too many editor actions. 3/2.',
      intent: 'save',
      limit: 2,
      observed: 3,
      instanceId: 'canvas-a',
      pageId: 'home'
    }
  ]);
  for (const request of state.requests) {
    expect(request).toMatchObject({
      intent: 'save',
      revision: 'rev-1',
      project_hash: 'hash-1',
      draft_token: 'draft-1',
      draft_generation: 1
    });
  }

  await resolveFetch(page, 1, 'rev-2', 2);
  await awaitSave(page, 1);
  await expect(root).not.toHaveAttribute('data-fly-browser-problem');
  await expect(alert).toHaveCount(0);

  await emitSave(page, 4);
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (globalThis as PendingIntentScope).__flyPendingFetches?.length ?? 0
      )
    )
    .toBe(3);
  await resolveFetch(page, 2, 'rev-3', 3);
  await awaitSave(page, 3);
  await resolveFetch(page, 0, 'rev-stale', 9);
  await awaitSave(page, 0);

  state = await readState(page);
  expect(state).toMatchObject({
    aborted: 0,
    problem: null,
    revision: 'rev-3',
    projectHash: 'hash-3',
    draft: { token: 'draft-3', generation: 3 }
  });
  expect(state.requests).toHaveLength(3);
  expect(state.rejected).toHaveLength(1);
  expect(state.responses).toHaveLength(3);
  expect(state.responses[0]).toMatchObject({
    status: 200,
    requestGeneration: 2,
    current: true
  });
  expect(state.responses[1]).toMatchObject({
    status: 200,
    requestGeneration: 3,
    current: true
  });
  expect(state.responses[2]).toMatchObject({
    status: 200,
    requestGeneration: 1,
    current: false
  });
});

test('unmount aborts every accepted in-flight intent without surfacing errors', async ({
  page
}) => {
  await mountPendingIntentContract(page);
  await emitSave(page, 1);
  await emitSave(page, 2);

  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (globalThis as PendingIntentScope).__flyPendingFetches?.length ?? 0
      )
    )
    .toBe(2);

  await page.evaluate(() => {
    (globalThis as PendingIntentScope).FlyBrowser?.unmountAll?.();
  });
  await awaitSave(page, 0);
  await awaitSave(page, 1);

  const state = await readState(page);
  expect(state).toMatchObject({
    aborted: 2,
    mounted: 'false',
    problem: null,
    revision: 'rev-1',
    projectHash: 'hash-1',
    draft: { token: 'draft-1', generation: 1 },
    rejected: [],
    responses: []
  });
  expect(state.errors).toHaveLength(2);
  expect(state.errors[0]).toMatchObject({
    requestGeneration: 1,
    current: false
  });
  expect(state.errors[1]).toMatchObject({
    requestGeneration: 2,
    current: false
  });
  await expect(page.locator('[data-fly-browser-status]')).toHaveCount(0);
});
