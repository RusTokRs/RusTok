import { expect, Page, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const adapterPath = resolve(
  process.cwd(),
  "../../crates/fly-browser/assets/fly-browser.js",
);

type PendingFetch = {
  signal: AbortSignal;
  resolve: (response: Response) => void;
};

type IntentAbort = {
  code?: string;
  kind?: string;
  requestGeneration?: number;
  current?: boolean;
};

type PendingScope = typeof globalThis & {
  __FLY_BROWSER_CONFIG__?: Record<string, unknown>;
  __flyAborts?: IntentAbort[];
  __flyErrors?: unknown[];
  __flyPendingFetches?: PendingFetch[];
  __flyProblems?: Array<{ code?: string }>;
  __flyRejected?: Array<{ code?: string; limit?: number; observed?: number }>;
  __flySavePromises?: Promise<unknown>[];
  FlyBrowser?: {
    mountAll?: (options?: Record<string, unknown>) => Array<{
      emitIntent: (
        intent: string,
        payload: Record<string, unknown>,
      ) => Promise<unknown>;
    }>;
    unmountAll?: () => void;
  };
};

async function mountPendingContract(page: Page) {
  const adapterSource = await readFile(adapterPath, "utf8");
  await page.setContent(`
    <div
      id="fly-root"
      data-fly-browser-root
      data-fly-page-id="home"
      data-fly-intent-endpoint="/fly-intent"
    >
      <iframe id="canvas-a-frame" data-fly-iframe-canvas title="Fly pending intent canvas"></iframe>
    </div>
  `);

  await page.evaluate(async (source) => {
    const scope = globalThis as PendingScope;
    const root = document.querySelector("#fly-root");
    if (!(root instanceof HTMLElement)) throw new Error("Fly root unavailable");

    scope.__FLY_BROWSER_CONFIG__ = {
      autoMount: true,
      intentEndpoint: "/fly-intent",
      maxPendingIntentRequests: 2,
      pendingIntentLimitMessage: "Too many editor actions.",
      intentRequestTimeoutMs: 10_000,
    };
    scope.__flyAborts = [];
    scope.__flyErrors = [];
    scope.__flyPendingFetches = [];
    scope.__flyProblems = [];
    scope.__flyRejected = [];
    scope.__flySavePromises = [];

    root.addEventListener("fly:browser-intent-aborted", (event) => {
      scope.__flyAborts?.push((event as CustomEvent<IntentAbort>).detail);
    });
    root.addEventListener("fly:browser-error", (event) => {
      scope.__flyErrors?.push((event as CustomEvent).detail);
    });
    root.addEventListener("fly:browser-problem", (event) => {
      scope.__flyProblems?.push((event as CustomEvent).detail);
    });
    root.addEventListener("fly:browser-intent-rejected", (event) => {
      scope.__flyRejected?.push((event as CustomEvent).detail);
    });

    globalThis.fetch = async (_input, init = {}) => {
      const signal = init.signal;
      if (!(signal instanceof AbortSignal)) {
        throw new Error("Intent request signal unavailable");
      }
      return new Promise<Response>((resolveResponse, rejectResponse) => {
        const rejectAborted = () => {
          rejectResponse(new DOMException("Aborted", "AbortError"));
        };
        if (signal.aborted) {
          rejectAborted();
          return;
        }
        signal.addEventListener("abort", rejectAborted, { once: true });
        scope.__flyPendingFetches?.push({ signal, resolve: resolveResponse });
      });
    };

    const url = URL.createObjectURL(
      new Blob([source], { type: "text/javascript" }),
    );
    try {
      await import(url);
    } finally {
      URL.revokeObjectURL(url);
    }
  }, adapterSource);

  await expect(page.locator("#fly-root")).toHaveAttribute(
    "data-fly-browser-mounted",
    "true",
  );
}

async function emitSave(page: Page, number: number) {
  await page.evaluate((saveNumber) => {
    const scope = globalThis as PendingScope;
    const adapter = scope.FlyBrowser?.mountAll?.(
      scope.__FLY_BROWSER_CONFIG__ ?? {},
    )[0];
    if (!adapter) throw new Error("Fly adapter unavailable");
    scope.__flySavePromises?.push(
      adapter.emitIntent("save", { save_number: saveNumber }),
    );
  }, number);
}

async function awaitSave(page: Page, index: number) {
  await page.evaluate(async (saveIndex) => {
    const promise = (globalThis as PendingScope).__flySavePromises?.[saveIndex];
    if (!promise) throw new Error(`Save promise ${saveIndex} unavailable`);
    await promise;
  }, index);
}

async function resolveFetch(page: Page, index: number) {
  await page.evaluate((fetchIndex) => {
    const pending = (globalThis as PendingScope).__flyPendingFetches?.[
      fetchIndex
    ];
    if (!pending) throw new Error(`Pending fetch ${fetchIndex} unavailable`);
    pending.resolve(
      new Response(JSON.stringify({ result: {} }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );
  }, index);
}

test("pending limit rejects newest work and releases settled slots", async ({
  page,
}) => {
  await mountPendingContract(page);
  await emitSave(page, 1);
  await emitSave(page, 2);
  await emitSave(page, 3);
  await awaitSave(page, 2);

  await expect
    .poll(() =>
      page.evaluate(
        () => (globalThis as PendingScope).__flyPendingFetches?.length ?? 0,
      ),
    )
    .toBe(2);

  const rejected = await page.evaluate(
    () => (globalThis as PendingScope).__flyRejected ?? [],
  );
  expect(rejected).toEqual([
    expect.objectContaining({
      code: "PENDING_INTENT_LIMIT",
      limit: 2,
      observed: 3,
    }),
  ]);
  await expect(page.locator("#fly-root")).toHaveAttribute(
    "data-fly-browser-problem",
    "PENDING_INTENT_LIMIT",
  );

  await resolveFetch(page, 1);
  await awaitSave(page, 1);
  await emitSave(page, 4);
  await expect
    .poll(() =>
      page.evaluate(
        () => (globalThis as PendingScope).__flyPendingFetches?.length ?? 0,
      ),
    )
    .toBe(3);

  await resolveFetch(page, 2);
  await awaitSave(page, 3);
  await resolveFetch(page, 0);
  await awaitSave(page, 0);
  await expect(page.locator("#fly-root")).not.toHaveAttribute(
    "data-fly-browser-problem",
  );
});

test("unmount emits typed adapter-stop aborts without browser errors", async ({
  page,
}) => {
  await mountPendingContract(page);
  await emitSave(page, 1);
  await emitSave(page, 2);

  await expect
    .poll(() =>
      page.evaluate(
        () => (globalThis as PendingScope).__flyPendingFetches?.length ?? 0,
      ),
    )
    .toBe(2);

  await page.evaluate(() => {
    (globalThis as PendingScope).FlyBrowser?.unmountAll?.();
  });
  await awaitSave(page, 0);
  await awaitSave(page, 1);

  const state = await page.evaluate(() => {
    const scope = globalThis as PendingScope;
    return {
      aborts: scope.__flyAborts ?? [],
      errors: scope.__flyErrors ?? [],
      problems: scope.__flyProblems ?? [],
    };
  });
  expect(state.errors).toEqual([]);
  expect(state.problems).toEqual([]);
  expect(state.aborts).toHaveLength(2);
  expect(state.aborts).toEqual([
    expect.objectContaining({
      code: "INTENT_REQUEST_ABORTED",
      kind: "adapter_stop",
      requestGeneration: 1,
      current: false,
    }),
    expect.objectContaining({
      code: "INTENT_REQUEST_ABORTED",
      kind: "adapter_stop",
      requestGeneration: 2,
      current: false,
    }),
  ]);
});
