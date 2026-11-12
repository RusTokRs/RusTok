import { expect, Page, test } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { openBlankDocument } from './support/fly-blank-document';

const adapterPath = resolve(
  process.cwd(),
  '../../crates/ui/fly/browser/assets/fly-browser.js'
);

const SAVE_FAILURE_KEY = 'fly:ssr-save-failure:home';

// The shape of `PagesBrowserIntentProblem`, which the Pages endpoint puts into `save_failure`.
const saveFailure = {
  status: 409,
  code: 'REVISION_CONFLICT',
  error: 'The page was changed on the server. Your edit is kept in the draft.',
  intent: 'save'
};

type FetchMode = 'rejected' | 'dirty' | 'saved' | 'denied';

type BrowserProblem = {
  status?: number;
  code?: string;
  error?: string;
  intent?: string | null;
};

type SaveFailureScope = typeof globalThis & {
  __FLY_BROWSER_CONFIG__?: Record<string, unknown>;
  __flyBrowserProblems?: BrowserProblem[];
  __flyFetchMode?: FetchMode;
  __flyBeforeReload?: boolean;
  FlyBrowser?: {
    mountAll?: (options?: Record<string, unknown>) => Array<{
      emitIntent: (
        intent: string,
        payload: Record<string, unknown>
      ) => Promise<unknown>;
    }>;
  };
};

// Storage needs a real origin, so the adapter is mounted in an empty document of the origin
// under test. No page of the host is loaded: see `openBlankDocument`.
async function openHost(page: Page) {
  await openBlankDocument(page);
}

// Mounts the adapter into the current document. `parked` is written to `sessionStorage` first,
// as an earlier document of this tab would have left it.
async function mountAdapter(
  page: Page,
  mode: FetchMode,
  parked?: Record<string, unknown>
) {
  const adapterSource = await readFile(adapterPath, 'utf8');
  await page.evaluate(
    async ({ source, mode, parked, failure, key }) => {
      document.body.innerHTML = `
        <div
          id="fly-root"
          data-fly-browser-root
          data-fly-page-id="home"
          data-fly-revision="rev-1"
          data-fly-project-hash="hash-1"
          data-fly-intent-endpoint="/fly-intent"
        >
          <iframe id="canvas-a-frame" data-fly-iframe-canvas title="Fly save failure canvas"></iframe>
        </div>
      `;

      const scope = globalThis as SaveFailureScope;
      const root = document.querySelector('#fly-root');
      if (!(root instanceof HTMLElement))
        throw new Error('Fly root unavailable');

      scope.__FLY_BROWSER_CONFIG__ = {
        autoMount: true,
        expectedOrigin: 'null',
        intentEndpoint: '/fly-intent'
      };
      scope.__flyBrowserProblems = [];
      scope.__flyFetchMode = mode;
      if (parked) sessionStorage.setItem(key, JSON.stringify(parked));

      root.addEventListener('fly:browser-problem', (event) => {
        scope.__flyBrowserProblems?.push(
          (event as CustomEvent<BrowserProblem>).detail
        );
      });

      const json = (status: number, body: unknown) =>
        new Response(JSON.stringify(body), {
          status,
          headers: { 'content-type': 'application/json' }
        });
      const originalFetch = globalThis.fetch;
      globalThis.fetch = async (input, init = {}) => {
        const url =
          typeof input === 'string'
            ? input
            : input instanceof Request
              ? input.url
              : String(input);
        if (!url.includes('/fly-intent')) return originalFetch(input, init);
        const draft = { draft_token: 'draft-2', draft_generation: 2 };
        const state = { revision_id: 'rev-1', project_hash: 'hash-1' };
        switch (scope.__flyFetchMode) {
          // The edit reached the draft, the save was rejected: HTTP 200 with the failure and a
          // request to reload.
          case 'rejected':
            return json(200, {
              result: { ...state, dirty: true },
              save_failure: failure,
              reload: true,
              ...draft
            });
          // An intent that leaves the document alone after a rejected save.
          case 'dirty':
            return json(200, { result: { ...state, dirty: true }, ...draft });
          case 'denied':
            return json(403, {
              status: 403,
              error: 'browser intent `save` requires editor capability `edit`',
              code: 'FLY_CAPABILITY_DENIED',
              intent: 'save'
            });
          default:
            return json(200, {
              result: {
                revision_id: 'rev-2',
                project_hash: 'hash-2',
                dirty: false
              },
              ...draft
            });
        }
      };

      const url = URL.createObjectURL(
        new Blob([source], { type: 'text/javascript' })
      );
      try {
        await import(url);
      } finally {
        URL.revokeObjectURL(url);
      }
    },
    {
      source: adapterSource,
      mode,
      parked,
      failure: saveFailure,
      key: SAVE_FAILURE_KEY
    }
  );

  await expect(page.locator('#fly-root')).toHaveAttribute(
    'data-fly-browser-mounted',
    'true'
  );
}

async function setFetchMode(page: Page, mode: FetchMode) {
  await page.evaluate((next) => {
    (globalThis as SaveFailureScope).__flyFetchMode = next;
  }, mode);
}

async function emit(page: Page, intent: string) {
  await page.evaluate(async (name) => {
    const scope = globalThis as SaveFailureScope;
    const adapter = scope.FlyBrowser?.mountAll?.(
      scope.__FLY_BROWSER_CONFIG__ ?? {}
    )[0];
    if (!adapter) throw new Error('Fly adapter unavailable');
    await adapter.emitIntent(name, {});
  }, intent);
}

async function reportedProblems(page: Page) {
  return page.evaluate(
    () => (globalThis as SaveFailureScope).__flyBrowserProblems ?? []
  );
}

async function parkedFailure(page: Page) {
  return page.evaluate(
    (key) => JSON.parse(sessionStorage.getItem(key) ?? 'null'),
    SAVE_FAILURE_KEY
  );
}

test('a rejected save is announced after the reload and stands until the working copy is saved', async ({
  page
}) => {
  await openHost(page);
  await mountAdapter(page, 'rejected');

  // The response asks for a reload; the document that reported the failure is discarded.
  await page.evaluate(() => {
    (globalThis as SaveFailureScope).__flyBeforeReload = true;
  });
  await emit(page, 'key_stroke').catch(() => {
    // The reload may destroy the context before the evaluation settles.
  });
  await page.waitForFunction(
    () =>
      (globalThis as SaveFailureScope).__flyBeforeReload !== true &&
      document.readyState === 'complete'
  );
  await page.waitForLoadState('networkidle');
  await expect(page.locator('#fly-root')).toHaveCount(0);
  expect(await parkedFailure(page)).toMatchObject(saveFailure);

  // The adapter of the new document reports it at mount, before any intent.
  await mountAdapter(page, 'dirty');
  const root = page.locator('#fly-root');
  const alert = page.locator('[data-fly-browser-status="problem"]');
  await expect(root).toHaveAttribute(
    'data-fly-browser-problem',
    'REVISION_CONFLICT'
  );
  await expect(root).toHaveAttribute('data-fly-save-failed', 'true');
  await expect(alert).toHaveAttribute('role', 'alert');
  await expect(alert).toHaveText(saveFailure.error);
  expect(await reportedProblems(page)).toMatchObject([saveFailure]);

  // An intent that leaves the working copy dirty neither clears the failure nor announces it
  // a second time.
  await emit(page, 'select');
  await expect(root).toHaveAttribute('data-fly-save-failed', 'true');
  await expect(alert).toHaveText(saveFailure.error);
  expect(await reportedProblems(page)).toHaveLength(1);
  expect(await parkedFailure(page)).toMatchObject(saveFailure);

  // A response with a saved working copy ends it.
  await setFetchMode(page, 'saved');
  await emit(page, 'save');
  await expect(root).not.toHaveAttribute('data-fly-browser-problem');
  await expect(root).not.toHaveAttribute('data-fly-save-failed');
  await expect(alert).toHaveCount(0);
  await expect(root).toHaveAttribute('data-fly-revision', 'rev-2');
  expect(await parkedFailure(page)).toBeNull();
  expect(await reportedProblems(page)).toHaveLength(1);
});

test('a standing save failure returns after another problem replaced it', async ({
  page
}) => {
  await openHost(page);
  await mountAdapter(page, 'denied', { at: Date.now(), ...saveFailure });

  const root = page.locator('#fly-root');
  const alert = page.locator('[data-fly-browser-status="problem"]');
  await expect(root).toHaveAttribute('data-fly-save-failed', 'true');

  await emit(page, 'save');
  await expect(root).toHaveAttribute(
    'data-fly-browser-problem',
    'FLY_CAPABILITY_DENIED'
  );
  await expect(root).not.toHaveAttribute('data-fly-save-failed');

  await setFetchMode(page, 'dirty');
  await emit(page, 'select');
  await expect(root).toHaveAttribute(
    'data-fly-browser-problem',
    'REVISION_CONFLICT'
  );
  await expect(root).toHaveAttribute('data-fly-save-failed', 'true');
  await expect(alert).toHaveText(saveFailure.error);
  expect((await reportedProblems(page)).map((problem) => problem.code)).toEqual(
    ['REVISION_CONFLICT', 'FLY_CAPABILITY_DENIED', 'REVISION_CONFLICT']
  );
});

test('a parked save failure older than ten minutes is dropped at mount', async ({
  page
}) => {
  await openHost(page);
  await mountAdapter(page, 'dirty', {
    at: Date.now() - 11 * 60 * 1000,
    ...saveFailure
  });

  const root = page.locator('#fly-root');
  await expect(root).not.toHaveAttribute('data-fly-browser-problem');
  await expect(root).not.toHaveAttribute('data-fly-save-failed');
  await expect(page.locator('[data-fly-browser-status="problem"]')).toHaveCount(
    0
  );
  expect(await reportedProblems(page)).toHaveLength(0);
  expect(await parkedFailure(page)).toBeNull();
});
