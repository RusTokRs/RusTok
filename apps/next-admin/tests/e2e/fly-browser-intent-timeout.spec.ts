import { expect, Page, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const adapterPath = resolve(
  process.cwd(),
  "../../crates/fly-browser/assets/fly-browser.js",
);

type IntentAbort = {
  code?: string;
  kind?: string;
  error?: string;
  requestGeneration?: number;
  current?: boolean;
};

type IntentTimeout = {
  code?: string;
  error?: string;
  timeoutMs?: number;
  requestGeneration?: number;
  current?: boolean;
};

type TimeoutScope = typeof globalThis & {
  __FLY_BROWSER_CONFIG__?: Record<string, unknown>;
  __flyAborts?: IntentAbort[];
  __flyErrors?: unknown[];
  __flyFetchMode?: "hang" | "success";
  __flyProblems?: Array<{ code?: string; error?: string }>;
  __flySavePromises?: Promise<unknown>[];
  __flyTimeouts?: IntentTimeout[];
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

async function mountTimeoutContract(page: Page, timeoutMs: number) {
  const adapterSource = await readFile(adapterPath, "utf8");
  await page.setContent(`
    <div
      id="fly-root"
      data-fly-browser-root
      data-fly-page-id="home"
      data-fly-intent-endpoint="/fly-intent"
    >
      <iframe id="canvas-a-frame" data-fly-iframe-canvas title="Fly timeout canvas"></iframe>
    </div>
  `);

  await page.evaluate(
    async ({ source, timeoutMs }) => {
      const scope = globalThis as TimeoutScope;
      const root = document.querySelector("#fly-root");
      if (!(root instanceof HTMLElement))
        throw new Error("Fly root unavailable");

      scope.__FLY_BROWSER_CONFIG__ = {
        autoMount: true,
        intentEndpoint: "/fly-intent",
        maxPendingIntentRequests: 1,
        intentRequestTimeoutMs: timeoutMs,
        intentRequestTimeoutMessage: "Editor save timed out",
      };
      scope.__flyAborts = [];
      scope.__flyErrors = [];
      scope.__flyFetchMode = "hang";
      scope.__flyProblems = [];
      scope.__flySavePromises = [];
      scope.__flyTimeouts = [];

      root.addEventListener("fly:browser-intent-aborted", (event) => {
        scope.__flyAborts?.push((event as CustomEvent<IntentAbort>).detail);
      });
      root.addEventListener("fly:browser-intent-timeout", (event) => {
        scope.__flyTimeouts?.push((event as CustomEvent<IntentTimeout>).detail);
      });
      root.addEventListener("fly:browser-error", (event) => {
        scope.__flyErrors?.push((event as CustomEvent).detail);
      });
      root.addEventListener("fly:browser-problem", (event) => {
        scope.__flyProblems?.push((event as CustomEvent).detail);
      });

      globalThis.fetch = async (_input, init = {}) => {
        if (scope.__flyFetchMode === "success") {
          return new Response(JSON.stringify({ result: {} }), {
            status: 200,
            headers: { "content-type": "application/json" },
          });
        }
        const signal = init.signal;
        if (!(signal instanceof AbortSignal)) {
          throw new Error("Intent request signal unavailable");
        }
        return new Promise<Response>((_resolve, reject) => {
          const rejectAborted = () => {
            reject(new DOMException("Aborted", "AbortError"));
          };
          if (signal.aborted) {
            rejectAborted();
            return;
          }
          signal.addEventListener("abort", rejectAborted, { once: true });
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
    },
    { source: adapterSource, timeoutMs },
  );

  await expect(page.locator("#fly-root")).toHaveAttribute(
    "data-fly-browser-mounted",
    "true",
  );
}

async function emitSave(page: Page) {
  await page.evaluate(() => {
    const scope = globalThis as TimeoutScope;
    const adapter = scope.FlyBrowser?.mountAll?.(
      scope.__FLY_BROWSER_CONFIG__ ?? {},
    )[0];
    if (!adapter) throw new Error("Fly adapter unavailable");
    scope.__flySavePromises?.push(adapter.emitIntent("save", {}));
  });
}

async function awaitSave(page: Page, index: number) {
  await page.evaluate(async (saveIndex) => {
    const promise = (globalThis as TimeoutScope).__flySavePromises?.[saveIndex];
    if (!promise) throw new Error(`Save promise ${saveIndex} unavailable`);
    await promise;
  }, index);
}

test("timeout emits typed abort, releases the slot and allows retry", async ({
  page,
}) => {
  await mountTimeoutContract(page, 40);
  await emitSave(page);
  await awaitSave(page, 0);

  await expect(page.locator("#fly-root")).toHaveAttribute(
    "data-fly-browser-problem",
    "INTENT_REQUEST_TIMEOUT",
  );
  await expect(page.locator('[data-fly-browser-status="problem"]')).toHaveText(
    "Editor save timed out after 40 ms.",
  );

  let state = await page.evaluate(() => {
    const scope = globalThis as TimeoutScope;
    return {
      aborts: scope.__flyAborts ?? [],
      errors: scope.__flyErrors ?? [],
      problems: scope.__flyProblems ?? [],
      timeouts: scope.__flyTimeouts ?? [],
    };
  });
  expect(state.errors).toEqual([]);
  expect(state.timeouts).toEqual([
    expect.objectContaining({
      code: "INTENT_REQUEST_TIMEOUT",
      timeoutMs: 40,
      requestGeneration: 1,
      current: true,
    }),
  ]);
  expect(state.aborts).toEqual([
    expect.objectContaining({
      code: "INTENT_REQUEST_TIMEOUT",
      kind: "timeout",
      requestGeneration: 1,
      current: true,
    }),
  ]);

  await page.evaluate(() => {
    (globalThis as TimeoutScope).__flyFetchMode = "success";
  });
  await emitSave(page);
  await awaitSave(page, 1);
  await expect(page.locator("#fly-root")).not.toHaveAttribute(
    "data-fly-browser-problem",
  );

  state = await page.evaluate(() => {
    const scope = globalThis as TimeoutScope;
    return {
      aborts: scope.__flyAborts ?? [],
      errors: scope.__flyErrors ?? [],
      timeouts: scope.__flyTimeouts ?? [],
    };
  });
  expect(state.errors).toEqual([]);
  expect(state.aborts).toHaveLength(1);
  expect(state.timeouts).toHaveLength(1);
});

test("unmount clears timer and emits only adapter-stop abort", async ({
  page,
}) => {
  await mountTimeoutContract(page, 80);
  await emitSave(page);
  await page.evaluate(() => {
    (globalThis as TimeoutScope).FlyBrowser?.unmountAll?.();
  });
  await awaitSave(page, 0);
  await page.waitForTimeout(120);

  const state = await page.evaluate(() => {
    const scope = globalThis as TimeoutScope;
    return {
      aborts: scope.__flyAborts ?? [],
      errors: scope.__flyErrors ?? [],
      problems: scope.__flyProblems ?? [],
      timeouts: scope.__flyTimeouts ?? [],
    };
  });
  expect(state.errors).toEqual([]);
  expect(state.problems).toEqual([]);
  expect(state.timeouts).toEqual([]);
  expect(state.aborts).toEqual([
    expect.objectContaining({
      code: "INTENT_REQUEST_ABORTED",
      kind: "adapter_stop",
      requestGeneration: 1,
      current: false,
    }),
  ]);
  await expect(page.locator("[data-fly-browser-status]")).toHaveCount(0);
});
