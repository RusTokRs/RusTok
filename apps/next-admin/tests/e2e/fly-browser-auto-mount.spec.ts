import { expect, Page, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const adapterPath = resolve(
  process.cwd(),
  "../../crates/fly-browser/assets/fly-browser.js",
);

type FlyAdapter = {
  lifecycleState: "created" | "started" | "stopped";
  start: () => FlyAdapter;
  stop: () => FlyAdapter;
};

type BrowserScope = typeof globalThis & {
  __FLY_BROWSER_CONFIG__?: Record<string, unknown>;
  __flyReadyEvents?: number;
  FlyBrowser?: {
    bootstrap?: (options?: Record<string, unknown>) => FlyAdapter[];
    mountAll?: (options?: Record<string, unknown>) => FlyAdapter[];
  };
};

async function loadManualBrowser(page: Page) {
  const adapterSource = await readFile(adapterPath, "utf8");

  await page.setContent(`
    <div
      id="fly-root"
      data-fly-browser-root
      data-fly-page-id="home"
      data-fly-expected-origin="null"
      style="position:relative"
    >
      <div style="position:relative">
        <iframe
          id="canvas-a-frame"
          data-fly-iframe-canvas
          title="Fly manual-mount canvas"
        ></iframe>
      </div>
    </div>
  `);

  await page.evaluate(async (source) => {
    const scope = globalThis as BrowserScope;
    const root = document.querySelector("#fly-root");
    const iframe = document.querySelector("#canvas-a-frame");
    if (!(root instanceof HTMLElement)) throw new Error("Fly root unavailable");
    if (!(iframe instanceof HTMLIFrameElement)) {
      throw new Error("Fly iframe unavailable");
    }

    const config = {
      autoMount: false,
      maxMessageBytes: 1024,
      maxGeometryComponents: 8,
    };
    scope.__FLY_BROWSER_CONFIG__ = config;
    scope.__flyReadyEvents = 0;
    root.addEventListener("fly:browser-ready", () => {
      scope.__flyReadyEvents = (scope.__flyReadyEvents ?? 0) + 1;
    });

    const url = URL.createObjectURL(
      new Blob([source], { type: "text/javascript" }),
    );
    try {
      await import(url);
    } finally {
      URL.revokeObjectURL(url);
    }

    const bootstrapResult = scope.FlyBrowser?.bootstrap?.(config) ?? [];
    if (bootstrapResult.length !== 0) {
      throw new Error("autoMount false must stay inert");
    }
    iframe.srcdoc = `<!doctype html>
      <html>
        <body>
          <script>
            parent.postMessage(JSON.stringify({
              protocol: "fly_iframe",
              instance_id: "canvas-a",
              sequence: 1,
              message: { type: "ready" }
            }), "*");
          <\/script>
        </body>
      </html>`;
  }, adapterSource);

  await expect(
    page.frameLocator("#canvas-a-frame").locator("body"),
  ).toHaveCount(1);
}

test("autoMount false supports an idempotent one-shot adapter lifecycle", async ({
  page,
}) => {
  await loadManualBrowser(page);

  const root = page.locator("#fly-root");
  await expect(root).not.toHaveAttribute("data-fly-browser-mounted");
  await expect(page.locator("[data-fly-browser-overlay]")).toHaveCount(0);

  const mountState = await page.evaluate(() => {
    const scope = globalThis as BrowserScope;
    const config = scope.__FLY_BROWSER_CONFIG__ ?? {};
    const first = scope.FlyBrowser?.mountAll?.(config) ?? [];
    const second = scope.FlyBrowser?.mountAll?.(config) ?? [];
    return {
      sameAdapter: first[0] === second[0],
      lifecycleState: first[0]?.lifecycleState,
      readyEvents: scope.__flyReadyEvents ?? 0,
    };
  });

  expect(mountState).toEqual({
    sameAdapter: true,
    lifecycleState: "started",
    readyEvents: 1,
  });
  await expect(root).toHaveAttribute("data-fly-browser-mounted", "true");
  await expect(page.locator("[data-fly-browser-overlay]")).toHaveCount(3);

  await page
    .frameLocator("#canvas-a-frame")
    .locator("body")
    .evaluate(() => {
      parent.postMessage(
        JSON.stringify({
          protocol: "fly_iframe",
          instance_id: "canvas-a",
          sequence: 2,
          message: { type: "ready" },
        }),
        "*",
      );
    });
  await expect(root).toHaveAttribute("data-fly-canvas-connected", "true");

  const lifecycle = await page.evaluate(() => {
    const scope = globalThis as BrowserScope;
    const config = scope.__FLY_BROWSER_CONFIG__ ?? {};
    const adapter = scope.FlyBrowser?.mountAll?.(config)[0];
    if (!adapter) throw new Error("Fly adapter unavailable");

    const repeatedStartIsSame = adapter.start() === adapter;
    const readyAfterRepeatedStart = scope.__flyReadyEvents ?? 0;
    const firstStopIsSame = adapter.stop() === adapter;
    const secondStopIsSame = adapter.stop() === adapter;

    let restartError: { name?: string; code?: string } | null = null;
    try {
      adapter.start();
    } catch (error) {
      restartError = {
        name: error instanceof Error ? error.name : undefined,
        code:
          typeof error === "object" && error !== null && "code" in error
            ? String(error.code)
            : undefined,
      };
    }

    const replacement = scope.FlyBrowser?.mountAll?.(config)[0];
    if (!replacement) throw new Error("replacement Fly adapter unavailable");
    const replacementIsFresh = replacement !== adapter;
    const replacementState = replacement.lifecycleState;
    const readyAfterReplacement = scope.__flyReadyEvents ?? 0;
    replacement.stop();

    return {
      repeatedStartIsSame,
      readyAfterRepeatedStart,
      firstStopIsSame,
      secondStopIsSame,
      restartError,
      replacementIsFresh,
      replacementState,
      readyAfterReplacement,
    };
  });

  expect(lifecycle).toEqual({
    repeatedStartIsSame: true,
    readyAfterRepeatedStart: 1,
    firstStopIsSame: true,
    secondStopIsSame: true,
    restartError: {
      name: "FlyBrowserLifecycleError",
      code: "ADAPTER_STOPPED",
    },
    replacementIsFresh: true,
    replacementState: "started",
    readyAfterReplacement: 2,
  });
  await expect(root).toHaveAttribute("data-fly-browser-mounted", "false");
  await expect(root).toHaveAttribute("data-fly-canvas-connected", "false");
  await expect(page.locator("[data-fly-browser-overlay]")).toHaveCount(0);
});
