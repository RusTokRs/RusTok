import { expect, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const adapterPath = resolve(
  process.cwd(),
  "../../crates/fly-browser/assets/fly-browser.js",
);

type FlyAdapter = {
  emitIntent: (
    intent: string,
    payload: Record<string, unknown>,
  ) => Promise<unknown>;
  stop: () => void;
};

type IsolationScope = typeof globalThis & {
  __FLY_BROWSER_CONFIG__?: Record<string, unknown>;
  FlyBrowser?: {
    mount?: (
      root: Element,
      options?: Record<string, unknown>,
    ) => FlyAdapter | null;
  };
};

test("parallel adapters keep abort signals request-scoped without replacing fetch", async ({
  page,
}) => {
  const adapterSource = await readFile(adapterPath, "utf8");

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
        const endpoint = typeof input === "string" ? input : String(input);
        const signal = init.signal;
        if (!(signal instanceof AbortSignal)) {
          throw new Error(`Missing request signal for ${endpoint}`);
        }
        requestSignals.push(signal);
        return new Promise<Response>((_resolve, reject) => {
          const rejectAborted = () => {
            abortedEndpoints.push(endpoint);
            reject(new DOMException("Aborted", "AbortError"));
          };
          if (signal.aborted) {
            rejectAborted();
            return;
          }
          signal.addEventListener("abort", rejectAborted, { once: true });
        });
      };

      Object.defineProperty(globalThis, "fetch", {
        configurable: true,
        get: () => fetchImplementation,
        set: (value) => {
          fetchAssignments += 1;
          fetchImplementation = value as typeof fetch;
        },
      });

      scope.__FLY_BROWSER_CONFIG__ = { autoMount: false };
      const url = URL.createObjectURL(
        new Blob([adapterSource], {
          type: "text/javascript",
        }),
      );
      try {
        await import(url);
      } finally {
        URL.revokeObjectURL(url);
      }

      const firstRoot = document.querySelector("#fly-root-a");
      const secondRoot = document.querySelector("#fly-root-b");
      if (!(firstRoot instanceof HTMLElement)) {
        throw new Error("First Fly root unavailable");
      }
      if (!(secondRoot instanceof HTMLElement)) {
        throw new Error("Second Fly root unavailable");
      }

      const firstAdapter = scope.FlyBrowser?.mount?.(firstRoot, {
        intentEndpoint: "/intent-a",
        maxPendingIntentRequests: 1,
        intentRequestTimeoutMs: 30,
      });
      const secondAdapter = scope.FlyBrowser?.mount?.(secondRoot, {
        intentEndpoint: "/intent-b",
        maxPendingIntentRequests: 1,
        intentRequestTimeoutMs: 1_000,
      });
      if (!firstAdapter || !secondAdapter) {
        throw new Error("Fly adapters unavailable");
      }

      const firstRequest = firstAdapter.emitIntent("save", { adapter: "a" });
      const secondRequest = secondAdapter.emitIntent("save", { adapter: "b" });
      await firstRequest;
      await new Promise((resolve) => globalThis.setTimeout(resolve, 50));

      const beforeStop = {
        abortedEndpoints: [...abortedEndpoints],
        firstProblem: firstRoot.dataset.flyBrowserProblem ?? null,
        secondProblem: secondRoot.dataset.flyBrowserProblem ?? null,
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
          requestSignals.length === 2 &&
          requestSignals[0] !== requestSignals[1],
      };
    },
    { adapterSource },
  );

  expect(state).toEqual({
    beforeStop: {
      abortedEndpoints: ["/intent-a"],
      firstProblem: "INTENT_REQUEST_TIMEOUT",
      secondProblem: null,
    },
    abortedEndpoints: ["/intent-a", "/intent-b"],
    fetchAssignments: 0,
    requestCount: 2,
    signalsDistinct: true,
  });
});
