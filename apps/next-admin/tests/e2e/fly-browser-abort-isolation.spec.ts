import { expect, test } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const adapterPath = resolve(
  process.cwd(),
  '../../crates/ui/fly/browser/assets/fly-browser.js'
);

type FlyAdapter = {
  emitIntent: (
    intent: string,
    payload: Record<string, unknown>
  ) => Promise<unknown>;
  stop: () => void;
};

type IsolationScope = typeof globalThis & {
  __FLY_BROWSER_CONFIG__?: Record<string, unknown>;
  FlyBrowser?: {
    mount?: (
      root: Element,
      options?: Record<string, unknown>
    ) => FlyAdapter | null;
  };
};

test('parallel adapters keep abort signals request-scoped without replacing fetch', async ({
  page
}) => {
  const adapterSource = await readFile(adapterPath, 'utf8');

  await page.setContent(`
    <div
      id="fly-root-a"
      data-fly-browser-root
      data-fly-page-id="page-a"
      data-fly-expected-origin="null"
      style="position:relative"
    >
      <div style="position:relative">
        <iframe
          id="canvas-a-frame"
          data-fly-iframe-canvas
          title="Fly abort isolation canvas A"
        ></iframe>
      </div>
    </div>
    <div
      id="fly-root-b"
      data-fly-browser-root
      data-fly-page-id="page-b"
      data-fly-expected-origin="null"
      style="position:relative"
    >
      <div style="position:relative">
        <iframe
          id="canvas-b-frame"
          data-fly-iframe-canvas
          title="Fly abort isolation canvas B"
        ></iframe>
      </div>
    </div>
  `);

  const state = await page.evaluate(
    async ({ adapterSource }) => {
      const scope = globalThis as IsolationScope;
      const abortedEndpoints: string[] = [];
      const requestSignals: AbortSignal[] = [];
      let fetchAssignments = 0;
      let fetchImplementation: typeof fetch = async (input, init = {}) => {
        const endpoint = typeof input === 'string' ? input : String(input);
        const signal = init.signal;
        if (!(signal instanceof AbortSignal)) {
          throw new Error(`Missing request signal for ${endpoint}`);
        }
        requestSignals.push(signal);
        return new Promise<Response>((_resolve, reject) => {
          const rejectAborted = () => {
            abortedEndpoints.push(endpoint);
            reject(new DOMException('Aborted', 'AbortError'));
          };
          if (signal.aborted) {
            rejectAborted();
            return;
          }
          signal.addEventListener('abort', rejectAborted, { once: true });
        });
      };

      Object.defineProperty(globalThis, 'fetch', {
        configurable: true,
        get: () => fetchImplementation,
        set: (value) => {
          fetchAssignments += 1;
          fetchImplementation = value as typeof fetch;
        }
      });

      scope.__FLY_BROWSER_CONFIG__ = { autoMount: false };
      const url = URL.createObjectURL(
        new Blob([adapterSource], {
          type: 'text/javascript'
        })
      );
      try {
        await import(url);
      } finally {
        URL.revokeObjectURL(url);
      }

      const firstRoot = document.querySelector('#fly-root-a');
      const secondRoot = document.querySelector('#fly-root-b');
      if (!(firstRoot instanceof HTMLElement)) {
        throw new Error('First Fly root unavailable');
      }
      if (!(secondRoot instanceof HTMLElement)) {
        throw new Error('Second Fly root unavailable');
      }

      const firstAdapter = scope.FlyBrowser?.mount?.(firstRoot, {
        intentEndpoint: '/intent-a',
        maxPendingIntentRequests: 1,
        intentRequestTimeoutMs: 30
      });
      const secondAdapter = scope.FlyBrowser?.mount?.(secondRoot, {
        intentEndpoint: '/intent-b',
        maxPendingIntentRequests: 1,
        intentRequestTimeoutMs: 1_000
      });
      if (!firstAdapter || !secondAdapter) {
        throw new Error('Fly adapters unavailable');
      }

      const firstRequest = firstAdapter.emitIntent('save', { adapter: 'a' });
      const secondRequest = secondAdapter.emitIntent('save', { adapter: 'b' });
      await firstRequest;
      await new Promise((resolve) => globalThis.setTimeout(resolve, 50));

      const beforeStop = {
        abortedEndpoints: [...abortedEndpoints],
        firstProblem: firstRoot.dataset.flyBrowserProblem ?? null,
        secondProblem: secondRoot.dataset.flyBrowserProblem ?? null
      };

      firstAdapter.stop();
      secondAdapter.stop();
      await secondRequest;

      return {
        beforeStop,
        abortedEndpoints,
        fetchAssignments,
        requestCount: requestSignals.length,
        signalsDistinct:
          requestSignals.length === 2 && requestSignals[0] !== requestSignals[1]
      };
    },
    { adapterSource }
  );

  expect(state).toEqual({
    beforeStop: {
      abortedEndpoints: ['/intent-a'],
      firstProblem: 'INTENT_REQUEST_TIMEOUT',
      secondProblem: null
    },
    abortedEndpoints: ['/intent-a', '/intent-b'],
    fetchAssignments: 0,
    requestCount: 2,
    signalsDistinct: true
  });
});


test('stopped adapter cannot emit or post and intent fetch refuses redirects', async ({
  page
}) => {
  const adapterSource = await readFile(adapterPath, 'utf8');
  await page.setContent(`
    <div id="fly-root" data-fly-browser-root data-fly-page-id="home"
      data-fly-expected-origin="null">
      <iframe id="canvas-frame" data-fly-iframe-canvas title="Lifecycle canvas"></iframe>
    </div>
  `);
  const state = await page.evaluate(async (source) => {
    const scope = globalThis as IsolationScope;
    scope.__FLY_BROWSER_CONFIG__ = { autoMount: false };
    const calls: Array<{ redirect: RequestRedirect | undefined; credentials: RequestCredentials | undefined }> = [];
    let emitted = 0;
    globalThis.fetch = async (_input, init = {}) => {
      calls.push({ redirect: init.redirect, credentials: init.credentials });
      return new Response(JSON.stringify({ result: { dirty: false } }), {
        status: 200,
        headers: { 'content-type': 'application/json' }
      });
    };
    const url = URL.createObjectURL(new Blob([source], { type: 'text/javascript' }));
    try {
      await import(url);
    } finally {
      URL.revokeObjectURL(url);
    }
    const root = document.querySelector('#fly-root');
    if (!(root instanceof HTMLElement)) throw new Error('Fly root unavailable');
    root.addEventListener('fly:browser-intent', () => { emitted += 1; });
    const adapter = scope.FlyBrowser?.mount?.(root, { intentEndpoint: '/intent' });
    if (!adapter) throw new Error('Fly adapter unavailable');
    await adapter.emitIntent('save', {});
    adapter.stop();
    const afterStop = await adapter.emitIntent('save', { stale: true });
    const direct = await (adapter as FlyAdapter & {
      postIntent: (input: Record<string, unknown>) => Promise<unknown>;
    }).postIntent({ intent: 'save', payload: { stale: true } });
    return { calls, emitted, afterStop, direct };
  }, adapterSource);
  expect(state).toEqual({
    calls: [{ redirect: 'error', credentials: 'same-origin' }],
    emitted: 1,
    afterStop: null,
    direct: null
  });
});



test('read-only intent cannot eclipse the acknowledgement of an earlier write', async ({
  page
}) => {
  const adapterSource = await readFile(adapterPath, 'utf8');
  await page.setContent(`
    <div id="fly-root" data-fly-browser-root data-fly-page-id="home"
      data-fly-revision="rev-1" data-fly-project-hash="old"
      data-fly-expected-origin="null">
      <iframe data-fly-iframe-canvas title="Ordering canvas"></iframe>
    </div>
  `);
  const evidence = await page.evaluate(async (source) => {
    const scope = globalThis as IsolationScope;
    scope.__FLY_BROWSER_CONFIG__ = { autoMount: false };
    const pending = new Map<string, (response: Response) => void>();
    globalThis.fetch = async (_url, options = {}) => {
      const intent = JSON.parse(String(options.body)).intent as string;
      return new Promise<Response>((resolve) => pending.set(intent, resolve));
    };
    const url = URL.createObjectURL(new Blob([source], { type: 'text/javascript' }));
    try {
      await import(url);
    } finally {
      URL.revokeObjectURL(url);
    }
    const root = document.querySelector('#fly-root');
    if (!(root instanceof HTMLElement)) throw new Error('Fly root missing');
    const adapter = scope.FlyBrowser?.mount?.(root, { intentEndpoint: '/intent' });
    if (!adapter) throw new Error('Fly adapter missing');
    const events: Array<{ intent: string; current: boolean }> = [];
    root.addEventListener('fly:browser-intent-response', (event) => {
      const detail = (event as CustomEvent<{
        request: { intent: string };
        current: boolean;
      }>).detail;
      events.push({ intent: detail.request.intent, current: detail.current });
    });
    const reply = (intent: string, revision: string) => {
      const resolve = pending.get(intent);
      if (!resolve) throw new Error(`No pending ${intent} request`);
      resolve(new Response(JSON.stringify({
        result: {
          revision_id: revision,
          project_hash: revision,
          dirty: revision === 'rev-1'
        }
      }), { status: 200, headers: { 'content-type': 'application/json' } }));
    };
    const write = adapter.emitIntent('save', {});
    const read = adapter.emitIntent('select', {});
    reply('select', 'rev-1');
    await read;
    reply('save', 'rev-2');
    await write;
    const state = {
      revision: root.dataset.flyRevision,
      hash: root.dataset.flyProjectHash,
      events
    };
    adapter.stop();
    return state;
  }, adapterSource);
  expect(evidence).toEqual({
    revision: 'rev-2',
    hash: 'rev-2',
    events: [
      { intent: 'select', current: false },
      { intent: 'save', current: true }
    ]
  });
});



test('mutating intents are serialized and rebase on the previous response', async ({
  page
}) => {
  const adapterSource = await readFile(adapterPath, 'utf8');
  await page.setContent(`
    <div id="fly-root" data-fly-browser-root data-fly-page-id="home"
      data-fly-revision="rev-1" data-fly-project-hash="hash-1"
      data-fly-expected-origin="null">
      <iframe data-fly-iframe-canvas title="Serialized canvas"></iframe>
    </div>
  `);
  const evidence = await page.evaluate(async (source) => {
    const scope = globalThis as IsolationScope;
    scope.__FLY_BROWSER_CONFIG__ = { autoMount: false };
    const requests: Array<{ revision: string; project_hash: string; intent: string }> = [];
    const pending: Array<(value: Response) => void> = [];
    globalThis.fetch = async (_url, options = {}) => {
      requests.push(JSON.parse(String(options.body)));
      return new Promise<Response>((resolve) => pending.push(resolve));
    };
    const url = URL.createObjectURL(new Blob([source], { type: 'text/javascript' }));
    try {
      await import(url);
    } finally {
      URL.revokeObjectURL(url);
    }
    const root = document.querySelector('#fly-root');
    if (!(root instanceof HTMLElement)) throw new Error('Fly root missing');
    const adapter = scope.FlyBrowser?.mount?.(root, { intentEndpoint: '/intent' });
    if (!adapter) throw new Error('Fly adapter missing');
    const first = adapter.emitIntent('patch_selected', { value: 'a' });
    const second = adapter.emitIntent('patch_selected', { value: 'b' });
    const simultaneousRequests = requests.length;
    pending[0](new Response(JSON.stringify({
      result: { revision_id: 'rev-2', project_hash: 'hash-2', dirty: true }
    }), { status: 200, headers: { 'content-type': 'application/json' } }));
    await first;
    const firstSettledRequests = requests.length;
    pending[1](new Response(JSON.stringify({
      result: { revision_id: 'rev-3', project_hash: 'hash-3', dirty: true }
    }), { status: 200, headers: { 'content-type': 'application/json' } }));
    await second;
    const state = {
      simultaneousRequests,
      firstSettledRequests,
      requests: requests.map(({ revision, project_hash, intent }) => ({
        revision, project_hash, intent
      })),
      finalRevision: root.dataset.flyRevision,
    };
    adapter.stop();
    return state;
  }, adapterSource);
  expect(evidence).toEqual({
    simultaneousRequests: 1,
    firstSettledRequests: 2,
    requests: [
      { revision: 'rev-1', project_hash: 'hash-1', intent: 'patch_selected' },
      { revision: 'rev-2', project_hash: 'hash-2', intent: 'patch_selected' }
    ],
    finalRevision: 'rev-3'
  });
});


test('stop resolves queued mutations without sending them', async ({ page }) => {
  const adapterSource = await readFile(adapterPath, 'utf8');
  await page.setContent(`
    <div id="fly-root" data-fly-browser-root data-fly-page-id="home"
      data-fly-expected-origin="null">
      <iframe data-fly-iframe-canvas title="Queued stop canvas"></iframe>
    </div>
  `);
  const result = await page.evaluate(async (source) => {
    const scope = globalThis as IsolationScope;
    scope.__FLY_BROWSER_CONFIG__ = { autoMount: false };
    let sends = 0;
    globalThis.fetch = async (_url, opts = {}) => {
      sends += 1;
      return new Promise<Response>((_resolve, reject) => {
        const signal = opts.signal;
        if (signal?.aborted) {
          reject(new DOMException('Aborted', 'AbortError'));
        } else {
          signal?.addEventListener('abort', () => {
            reject(new DOMException('Aborted', 'AbortError'));
          }, { once: true });
        }
      });
    };
    const url = URL.createObjectURL(new Blob([source], { type: 'text/javascript' }));
    try {
      await import(url);
    } finally {
      URL.revokeObjectURL(url);
    }
    const root = document.querySelector('#fly-root');
    if (!(root instanceof HTMLElement)) throw new Error('Fly root missing');
    const adapter = scope.FlyBrowser?.mount?.(root, { intentEndpoint: '/intent' });
    if (!adapter) throw new Error('Fly adapter missing');
    const first = adapter.emitIntent('save', {});
    const second = adapter.emitIntent('save', {});
    adapter.stop();
    return { sends, first: await first, second: await second };
  }, adapterSource);
  expect(result).toEqual({ sends: 1, first: null, second: null });
});



test('a failed or unconfirmed mutation never releases queued writes onto a stale base', async ({
  page
}) => {
  const adapterSource = await readFile(adapterPath, 'utf8');
  await page.setContent(`
    <div id="fly-root" data-fly-browser-root data-fly-page-id="home"
      data-fly-revision="rev-1" data-fly-project-hash="hash-1"
      data-fly-expected-origin="null">
      <iframe data-fly-iframe-canvas title="Failed queue canvas"></iframe>
    </div>
  `);

  const evidence = await page.evaluate(async (source) => {
    const scope = globalThis as IsolationScope;
    scope.__FLY_BROWSER_CONFIG__ = { autoMount: false };
    const deferred: Array<(value: Response) => void> = [];
    const requests: string[] = [];
    globalThis.fetch = async (_url, options = {}) => {
      requests.push(JSON.parse(String(options.body)).intent);
      return new Promise<Response>((resolve) => deferred.push(resolve));
    };
    const url = URL.createObjectURL(new Blob([source], { type: 'text/javascript' }));
    try {
      await import(url);
    } finally {
      URL.revokeObjectURL(url);
    }
    const root = document.querySelector('#fly-root');
    if (!(root instanceof HTMLElement)) throw new Error('Fly root missing');
    const adapter = scope.FlyBrowser?.mount?.(root, { intentEndpoint: '/intent' });
    if (!adapter) throw new Error('Fly adapter missing');
    const first = adapter.emitIntent('patch_selected', { value: 'first' });
    const second = adapter.emitIntent('patch_selected', { value: 'second' });
    deferred[0](new Response(JSON.stringify({
      code: 'REVISION_CONFLICT', error: 'Remote draft changed'
    }), { status: 409, headers: { 'content-type': 'application/json' } }));
    await first;
    const secondResult = await second;
    const outcome = {
      requests,
      secondResult,
      revision: root.dataset.flyRevision,
      status: root.dataset.flyNetworkStatus
    };
    adapter.stop();
    return outcome;
  }, adapterSource);

  expect(evidence).toEqual({
    requests: ['patch_selected'],
    secondResult: null,
    revision: 'rev-1',
    status: 'error'
  });
});

test('a successful HTTP response without a revision cannot replay the next queued mutation', async ({
  page
}) => {
  const adapterSource = await readFile(adapterPath, 'utf8');
  await page.setContent(`
    <div id="fly-root" data-fly-browser-root data-fly-page-id="home"
      data-fly-revision="rev-1" data-fly-project-hash="hash-1"
      data-fly-expected-origin="null">
      <iframe data-fly-iframe-canvas title="Unconfirmed queue canvas"></iframe>
    </div>
  `);
  const evidence = await page.evaluate(async (source) => {
    const scope = globalThis as IsolationScope;
    scope.__FLY_BROWSER_CONFIG__ = { autoMount: false };
    const deferred: Array<(value: Response) => void> = [];
    let sends = 0;
    globalThis.fetch = async () => {
      sends += 1;
      return new Promise<Response>((resolve) => deferred.push(resolve));
    };
    const url = URL.createObjectURL(new Blob([source], { type: 'text/javascript' }));
    try {
      await import(url);
    } finally {
      URL.revokeObjectURL(url);
    }
    const root = document.querySelector('#fly-root');
    if (!(root instanceof HTMLElement)) throw new Error('Fly root missing');
    const adapter = scope.FlyBrowser?.mount?.(root, { intentEndpoint: '/intent' });
    if (!adapter) throw new Error('Fly adapter missing');
    const first = adapter.emitIntent('save', {});
    const queued = adapter.emitIntent('patch_selected', {});
    deferred[0](new Response(JSON.stringify({ result: { dirty: true } }), {
      status: 200, headers: { 'content-type': 'application/json' }
    }));
    await first;
    const queuedResult = await queued;
    const result = { sends, queuedResult, status: root.dataset.flyNetworkStatus };
    adapter.stop();
    return result;
  }, adapterSource);
  expect(evidence).toEqual({
    sends: 1, queuedResult: null, status: 'error'
  });
});
