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

type BrowserResponseDetail = {
  ok?: boolean;
  status?: number;
  result?: { code?: string };
  requestGeneration?: number;
  current?: boolean;
};

type BrowserProblem = {
  code?: string;
  error?: string;
};

type PendingFetch = {
  request: BrowserRequest;
  resolve: (response: Response) => void;
};

type BrowserTestScope = typeof globalThis & {
  __FLY_BROWSER_CONFIG__?: Record<string, unknown>;
  __flyPendingFetches?: PendingFetch[];
  __flyProblems?: BrowserProblem[];
  __flyRequests?: BrowserRequest[];
  __flyResponses?: BrowserResponseDetail[];
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

async function mountResponseOrderContract(page: Page) {
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
          title="Fly response-order canvas"
        ></iframe>
      </div>
    </div>
  `);

  await page.evaluate(
    async ({ adapterSource, hardeningSource }) => {
      const root = document.querySelector('#fly-root');
      if (!(root instanceof HTMLElement)) throw new Error('Fly root unavailable');

      const scope = globalThis as BrowserTestScope;
      scope.__FLY_BROWSER_CONFIG__ = {
        autoMount: true,
        intentEndpoint: '/fly-intent'
      };
      scope.__flyPendingFetches = [];
      scope.__flyProblems = [];
      scope.__flyRequests = [];
      scope.__flyResponses = [];
      scope.__flySavePromises = [];
      sessionStorage.setItem(
        'fly:ssr-draft:home',
        JSON.stringify({ token: 'draft-1', generation: 1 })
      );

      root.addEventListener('fly:browser-intent-response', (event) => {
        scope.__flyResponses?.push(
          (event as CustomEvent<BrowserResponseDetail>).detail
        );
      });
      root.addEventListener('fly:browser-problem', (event) => {
        scope.__flyProblems?.push(
          (event as CustomEvent<BrowserProblem>).detail
        );
      });

      globalThis.fetch = async (_input, init = {}) => {
        const request =
          typeof init.body === 'string'
            ? (JSON.parse(init.body) as BrowserRequest)
            : {};
        scope.__flyRequests?.push(request);
        return new Promise<Response>((resolveResponse) => {
          scope.__flyPendingFetches?.push({
            request,
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

async function beginConcurrentSaves(page: Page, count = 2) {
  await page.evaluate((saveCount) => {
    const scope = globalThis as BrowserTestScope;
    const adapter = scope.FlyBrowser?.mountAll?.(
      scope.__FLY_BROWSER_CONFIG__ ?? {}
    )[0];
    if (!adapter) throw new Error('Fly adapter unavailable');
    for (let index = 0; index < saveCount; index += 1) {
      scope.__flySavePromises?.push(
        adapter.emitIntent('save', {
          project: { pages: [] },
          save_number: index + 1
        })
      );
    }
  }, count);

  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (globalThis as BrowserTestScope).__flyPendingFetches?.length ?? 0
      )
    )
    .toBe(count);
}

async function resolveFetch(
  page: Page,
  index: number,
  response: {
    status: number;
    body: Record<string, unknown>;
  }
) {
  await page.evaluate(
    ({ index, response }) => {
      const pending = (globalThis as BrowserTestScope).__flyPendingFetches?.[
        index
      ];
      if (!pending) throw new Error(`Pending fetch ${index} unavailable`);
      pending.resolve(
        new Response(JSON.stringify(response.body), {
          status: response.status,
          headers: { 'content-type': 'application/json' }
        })
      );
    },
    { index, response }
  );
}

async function awaitSave(page: Page, index: number) {
  await page.evaluate(async (saveIndex) => {
    const promise = (globalThis as BrowserTestScope).__flySavePromises?.[
      saveIndex
    ];
    if (!promise) throw new Error(`Save promise ${saveIndex} unavailable`);
    await promise;
  }, index);
}

async function readState(page: Page) {
  return page.evaluate(() => {
    const scope = globalThis as BrowserTestScope;
    const root = document.querySelector('#fly-root');
    return {
      mounted: root?.getAttribute('data-fly-browser-mounted'),
      problem: root?.getAttribute('data-fly-browser-problem'),
      revision: root?.getAttribute('data-fly-revision'),
      projectHash: root?.getAttribute('data-fly-project-hash'),
      draft: JSON.parse(
        sessionStorage.getItem('fly:ssr-draft:home') ?? 'null'
      ),
      problems: scope.__flyProblems ?? [],
      requests: scope.__flyRequests ?? [],
      responses: scope.__flyResponses ?? []
    };
  });
}

const denial = {
  status: 403,
  body: {
    status: 403,
    error: 'browser intent `save` requires editor capability `publish`',
    code: 'FLY_CAPABILITY_DENIED',
    intent: 'save',
    capability: 'publish',
    required: ['publish'],
    missing: ['publish']
  }
};

const success = {
  status: 200,
  body: {
    result: { revision_id: 'rev-2', project_hash: 'hash-2' },
    draft_token: 'draft-2',
    draft_generation: 2
  }
};

test('late denial cannot replace a newer successful response', async ({ page }) => {
  await mountResponseOrderContract(page);
  await beginConcurrentSaves(page);

  await resolveFetch(page, 1, success);
  await awaitSave(page, 1);
  let state = await readState(page);
  expect(state).toMatchObject({
    problem: null,
    revision: 'rev-2',
    projectHash: 'hash-2',
    draft: { token: 'draft-2', generation: 2 }
  });

  await resolveFetch(page, 0, denial);
  await awaitSave(page, 0);
  state = await readState(page);
  expect(state).toMatchObject({
    problem: null,
    revision: 'rev-2',
    projectHash: 'hash-2',
    draft: { token: 'draft-2', generation: 2 },
    problems: []
  });
  expect(state.requests).toHaveLength(2);
  expect(state.requests[0]).toMatchObject({
    revision: 'rev-1',
    project_hash: 'hash-1',
    draft_token: 'draft-1',
    draft_generation: 1
  });
  expect(state.requests[1]).toMatchObject({
    revision: 'rev-1',
    project_hash: 'hash-1',
    draft_token: 'draft-1',
    draft_generation: 1
  });
  expect(state.responses).toHaveLength(2);
  expect(state.responses[0]).toMatchObject({
    status: 200,
    requestGeneration: 2,
    current: true
  });
  expect(state.responses[1]).toMatchObject({
    status: 403,
    requestGeneration: 1,
    current: false
  });
  await expect(
    page.locator('[data-fly-browser-status="problem"]')
  ).toHaveCount(0);
});

test('late success cannot clear a newer denial or advance local state', async ({
  page
}) => {
  await mountResponseOrderContract(page);
  await beginConcurrentSaves(page);

  await resolveFetch(page, 1, denial);
  await awaitSave(page, 1);
  const alert = page.locator('[data-fly-browser-status="problem"]');
  await expect(page.locator('#fly-root')).toHaveAttribute(
    'data-fly-browser-problem',
    'FLY_CAPABILITY_DENIED'
  );
  await expect(alert).toHaveAttribute('role', 'alert');
  await expect(alert).toHaveText(
    'browser intent `save` requires editor capability `publish`'
  );

  await resolveFetch(page, 0, success);
  await awaitSave(page, 0);
  const state = await readState(page);
  expect(state).toMatchObject({
    problem: 'FLY_CAPABILITY_DENIED',
    revision: 'rev-1',
    projectHash: 'hash-1',
    draft: { token: 'draft-1', generation: 1 }
  });
  expect(state.problems).toHaveLength(1);
  expect(state.problems[0]).toMatchObject({ code: 'FLY_CAPABILITY_DENIED' });
  expect(state.responses).toHaveLength(2);
  expect(state.responses[0]).toMatchObject({
    status: 403,
    requestGeneration: 2,
    current: true
  });
  expect(state.responses[1]).toMatchObject({
    status: 200,
    requestGeneration: 1,
    current: false
  });
  await expect(alert).toHaveCount(1);
});

test('unmount invalidates an in-flight successful response', async ({ page }) => {
  await mountResponseOrderContract(page);
  await beginConcurrentSaves(page, 1);

  await page.evaluate(() => {
    (globalThis as BrowserTestScope).FlyBrowser?.unmountAll?.();
  });
  await resolveFetch(page, 0, success);
  await awaitSave(page, 0);

  const state = await readState(page);
  expect(state).toMatchObject({
    mounted: 'false',
    problem: null,
    revision: 'rev-1',
    projectHash: 'hash-1',
    draft: { token: 'draft-1', generation: 1 },
    problems: []
  });
  expect(state.responses).toHaveLength(1);
  expect(state.responses[0]).toMatchObject({
    status: 200,
    requestGeneration: 1,
    current: false
  });
  await expect(page.locator('[data-fly-browser-status]')).toHaveCount(0);
});
