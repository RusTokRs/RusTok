import { expect, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const adapterPath = resolve(
  process.cwd(),
  "../../crates/fly-browser/assets/fly-browser.js",
);
const hardeningPath = resolve(
  process.cwd(),
  "../../crates/fly-browser/assets/browser_hardening.js",
);

type IntentAbort = {
  code?: string;
  kind?: string;
  error?: string;
  intent?: string | null;
  requestGeneration?: number | null;
  current?: boolean;
  instanceId?: string;
  pageId?: string | null;
};

type BrowserFailure = {
  code?: string;
  error?: string;
  current?: boolean;
};

type FlyAdapter = {
  emitIntent: (
    intent: string,
    payload: Record<string, unknown>,
  ) => Promise<unknown>;
  postIntent: (
    input: Record<string, unknown>,
    options?: { signal?: AbortSignal },
  ) => Promise<unknown>;
  stop: () => void;
};

type ClassificationScope = typeof globalThis & {
  __FLY_BROWSER_CONFIG__?: Record<string, unknown>;
  FlyBrowser?: {
    mount?: (
      root: Element,
      options?: Record<string, unknown>,
    ) => FlyAdapter | null;
  };
};

test("expected aborts stay separate from network failures", async ({
  page,
}) => {
  const [adapterSource, hardeningSource] = await Promise.all([
    readFile(adapterPath, "utf8"),
    readFile(hardeningPath, "utf8"),
  ]);

  await page.setContent(`
    <div id="timeout-root" data-fly-browser-root data-fly-page-id="timeout-page">
      <iframe id="timeout-frame" data-fly-iframe-canvas title="Timeout canvas"></iframe>
    </div>
    <div id="stop-root" data-fly-browser-root data-fly-page-id="stop-page">
      <iframe id="stop-frame" data-fly-iframe-canvas title="Stop canvas"></iframe>
    </div>
    <div id="network-root" data-fly-browser-root data-fly-page-id="network-page">
      <iframe id="network-frame" data-fly-iframe-canvas title="Network canvas"></iframe>
    </div>
    <div id="external-root" data-fly-browser-root data-fly-page-id="external-page">
      <iframe id="external-frame" data-fly-iframe-canvas title="External abort canvas"></iframe>
    </div>
  `);

  const state = await page.evaluate(
    async ({ adapterSource, hardeningSource }) => {
      const scope = globalThis as ClassificationScope;
      const roots = {
        timeout: document.querySelector("#timeout-root"),
        stop: document.querySelector("#stop-root"),
        network: document.querySelector("#network-root"),
        external: document.querySelector("#external-root"),
      };
      if (
        !(roots.timeout instanceof HTMLElement) ||
        !(roots.stop instanceof HTMLElement) ||
        !(roots.network instanceof HTMLElement) ||
        !(roots.external instanceof HTMLElement)
      ) {
        throw new Error("Fly browser roots unavailable");
      }

      const events = {
        timeout: {
          aborts: [] as IntentAbort[],
          errors: [] as BrowserFailure[],
          problems: [] as BrowserFailure[],
        },
        stop: {
          aborts: [] as IntentAbort[],
          errors: [] as BrowserFailure[],
          problems: [] as BrowserFailure[],
        },
        network: {
          aborts: [] as IntentAbort[],
          errors: [] as BrowserFailure[],
          problems: [] as BrowserFailure[],
        },
        external: {
          aborts: [] as IntentAbort[],
          errors: [] as BrowserFailure[],
          problems: [] as BrowserFailure[],
        },
      };

      for (const name of ["timeout", "stop", "network", "external"] as const) {
        const root = roots[name];
        root.addEventListener("fly:browser-intent-aborted", (event) => {
          events[name].aborts.push((event as CustomEvent<IntentAbort>).detail);
        });
        root.addEventListener("fly:browser-error", (event) => {
          events[name].errors.push(
            (event as CustomEvent<BrowserFailure>).detail,
          );
        });
        root.addEventListener("fly:browser-problem", (event) => {
          events[name].problems.push(
            (event as CustomEvent<BrowserFailure>).detail,
          );
        });
      }

      globalThis.fetch = async (input, init = {}) => {
        const endpoint = typeof input === "string" ? input : String(input);
        if (endpoint === "/network") throw new Error("offline");
        const signal = init.signal;
        if (!(signal instanceof AbortSignal)) {
          throw new Error(`Missing abort signal for ${endpoint}`);
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

      scope.__FLY_BROWSER_CONFIG__ = { autoMount: false };
      const url = URL.createObjectURL(
        new Blob([adapterSource, hardeningSource], {
          type: "text/javascript",
        }),
      );
      try {
        await import(url);
      } finally {
        URL.revokeObjectURL(url);
      }

      const timeoutAdapter = scope.FlyBrowser?.mount?.(roots.timeout, {
        intentEndpoint: "/timeout",
        intentRequestTimeoutMs: 30,
      });
      const stopAdapter = scope.FlyBrowser?.mount?.(roots.stop, {
        intentEndpoint: "/stop",
        intentRequestTimeoutMs: 1_000,
      });
      const networkAdapter = scope.FlyBrowser?.mount?.(roots.network, {
        intentEndpoint: "/network",
        intentRequestTimeoutMs: 1_000,
      });
      const externalAdapter = scope.FlyBrowser?.mount?.(roots.external, {
        intentEndpoint: "/external",
        intentRequestTimeoutMs: 1_000,
      });
      if (
        !timeoutAdapter ||
        !stopAdapter ||
        !networkAdapter ||
        !externalAdapter
      ) {
        throw new Error("Fly browser adapters unavailable");
      }

      const timeoutRequest = timeoutAdapter.emitIntent("save", {
        source: "timeout",
      });
      const stopRequest = stopAdapter.emitIntent("save", { source: "stop" });
      const networkRequest = networkAdapter.emitIntent("save", {
        source: "network",
      });
      const externalController = new AbortController();
      const externalRequest = externalAdapter.postIntent(
        { intent: "save", payload: { source: "external" } },
        { signal: externalController.signal },
      );
      stopAdapter.stop();
      externalController.abort("navigation");

      await Promise.all([
        timeoutRequest,
        stopRequest,
        networkRequest,
        externalRequest,
      ]);
      await new Promise((resolve) => globalThis.setTimeout(resolve, 20));
      timeoutAdapter.stop();
      networkAdapter.stop();
      externalAdapter.stop();

      return events;
    },
    { adapterSource, hardeningSource },
  );

  expect(state.timeout.aborts).toHaveLength(1);
  expect(state.timeout.aborts[0]).toMatchObject({
    code: "INTENT_REQUEST_TIMEOUT",
    kind: "timeout",
    intent: "save",
    current: true,
    pageId: "timeout-page",
  });
  expect(state.timeout.problems.map((problem) => problem.code)).toEqual([
    "INTENT_REQUEST_TIMEOUT",
  ]);
  expect(
    state.timeout.problems.some((problem) => problem.code === "NETWORK_ERROR"),
  ).toBe(false);
  expect(state.timeout.errors).toEqual([]);

  expect(state.stop.aborts).toHaveLength(1);
  expect(state.stop.aborts[0]).toMatchObject({
    code: "INTENT_REQUEST_ABORTED",
    kind: "adapter_stop",
    intent: "save",
    current: false,
    pageId: "stop-page",
  });
  expect(state.stop.problems).toEqual([]);
  expect(state.stop.errors).toEqual([]);

  expect(state.network.aborts).toEqual([]);
  expect(state.network.problems).toHaveLength(1);
  expect(state.network.problems[0]).toMatchObject({
    code: "NETWORK_ERROR",
  });
  expect(state.network.errors).toHaveLength(1);

  expect(state.external.aborts).toHaveLength(1);
  expect(state.external.aborts[0]).toMatchObject({
    code: "INTENT_REQUEST_ABORTED",
    kind: "external",
    error: "navigation",
    intent: "save",
    current: true,
    pageId: "external-page",
  });
  expect(state.external.problems).toEqual([]);
  expect(state.external.errors).toEqual([]);
});
