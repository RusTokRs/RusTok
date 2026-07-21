import { expect, Page, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const adapterPath = resolve(
  process.cwd(),
  "../../crates/fly-browser/assets/fly-browser.js",
);

type PendingFetch = {
  resolve: (response: Response) => void;
};

type BrowserProblem = { code?: string };
type BrowserResponse = {
  status?: number;
  requestGeneration?: number;
  current?: boolean;
};

type OrderScope = typeof globalThis & {
  __FLY_BROWSER_CONFIG__?: Record<string, unknown>;
  __flyPendingFetches?: PendingFetch[];
  __flyProblems?: BrowserProblem[];
  __flyResponses?: BrowserResponse[];
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

const denial = {
  status: 403,
  body: {
    status: 403,
    error: "browser intent `save` requires editor capability `publish`",
    code: "FLY_CAPABILITY_DENIED",
    intent: "save",
    capability: "publish",
    required: ["publish"],
    missing: ["publish"],
  },
};

const success = {
  status: 200,
  body: {
    result: { revision_id: "rev-2", project_hash: "hash-2" },
    draft_token: "draft-2",
    draft_generation: 2,
  },
};

async function mountOrderContract(page: Page) {
  const adapterSource = await readFile(adapterPath, "utf8");
  await page.setContent(`
    <div
      id="fly-root"
      data-fly-browser-root
      data-fly-page-id="home"
      data-fly-revision="rev-1"
      data-fly-project-hash="hash-1"
      data-fly-intent-endpoint="/fly-intent"
    >
      <iframe id="canvas-a-frame" data-fly-iframe-canvas title="Fly response order canvas"></iframe>
    </div>
  `);

  await page.evaluate(async (source) => {
    const scope = globalThis as OrderScope;
    const root = document.querySelector("#fly-root");
    if (!(root instanceof HTMLElement)) throw new Error("Fly root unavailable");

    scope.__FLY_BROWSER_CONFIG__ = {
      autoMount: true,
      intentEndpoint: "/fly-intent",
    };
    scope.__flyPendingFetches = [];
    scope.__flyProblems = [];
    scope.__flyResponses = [];
    scope.__flySavePromises = [];
    sessionStorage.setItem(
      "fly:ssr-draft:home",
      JSON.stringify({ token: "draft-1", generation: 1 }),
    );

    root.addEventListener("fly:browser-problem", (event) => {
      scope.__flyProblems?.push((event as CustomEvent<BrowserProblem>).detail);
    });
    root.addEventListener("fly:browser-intent-response", (event) => {
      scope.__flyResponses?.push(
        (event as CustomEvent<BrowserResponse>).detail,
      );
    });

    globalThis.fetch = async () =>
      new Promise<Response>((resolveResponse) => {
        scope.__flyPendingFetches?.push({ resolve: resolveResponse });
      });

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

async function beginSaves(page: Page, count = 2) {
  await page.evaluate((saveCount) => {
    const scope = globalThis as OrderScope;
    const adapter = scope.FlyBrowser?.mountAll?.(
      scope.__FLY_BROWSER_CONFIG__ ?? {},
    )[0];
    if (!adapter) throw new Error("Fly adapter unavailable");
    for (let index = 0; index < saveCount; index += 1) {
      scope.__flySavePromises?.push(
        adapter.emitIntent("save", { save_number: index + 1 }),
      );
    }
  }, count);
  await expect
    .poll(() =>
      page.evaluate(
        () => (globalThis as OrderScope).__flyPendingFetches?.length ?? 0,
      ),
    )
    .toBe(count);
}

async function resolveFetch(
  page: Page,
  index: number,
  response: { status: number; body: Record<string, unknown> },
) {
  await page.evaluate(
    ({ index, response }) => {
      const pending = (globalThis as OrderScope).__flyPendingFetches?.[index];
      if (!pending) throw new Error(`Pending fetch ${index} unavailable`);
      pending.resolve(
        new Response(JSON.stringify(response.body), {
          status: response.status,
          headers: { "content-type": "application/json" },
        }),
      );
    },
    { index, response },
  );
}

async function awaitSave(page: Page, index: number) {
  await page.evaluate(async (saveIndex) => {
    const promise = (globalThis as OrderScope).__flySavePromises?.[saveIndex];
    if (!promise) throw new Error(`Save promise ${saveIndex} unavailable`);
    await promise;
  }, index);
}

async function readState(page: Page) {
  return page.evaluate(() => {
    const scope = globalThis as OrderScope;
    const root = document.querySelector("#fly-root");
    return {
      mounted: root?.getAttribute("data-fly-browser-mounted"),
      problem: root?.getAttribute("data-fly-browser-problem"),
      revision: root?.getAttribute("data-fly-revision"),
      projectHash: root?.getAttribute("data-fly-project-hash"),
      draft: JSON.parse(sessionStorage.getItem("fly:ssr-draft:home") ?? "null"),
      problems: scope.__flyProblems ?? [],
      responses: scope.__flyResponses ?? [],
    };
  });
}

test("late denial cannot replace newer success", async ({ page }) => {
  await mountOrderContract(page);
  await beginSaves(page);

  await resolveFetch(page, 1, success);
  await awaitSave(page, 1);
  await resolveFetch(page, 0, denial);
  await awaitSave(page, 0);

  const state = await readState(page);
  expect(state).toMatchObject({
    problem: null,
    revision: "rev-2",
    projectHash: "hash-2",
    draft: { token: "draft-2", generation: 2 },
    problems: [],
  });
  expect(state.responses).toEqual([
    expect.objectContaining({
      status: 200,
      requestGeneration: 2,
      current: true,
    }),
    expect.objectContaining({
      status: 403,
      requestGeneration: 1,
      current: false,
    }),
  ]);
});

test("late success cannot clear newer denial", async ({ page }) => {
  await mountOrderContract(page);
  await beginSaves(page);

  await resolveFetch(page, 1, denial);
  await awaitSave(page, 1);
  await resolveFetch(page, 0, success);
  await awaitSave(page, 0);

  const state = await readState(page);
  expect(state).toMatchObject({
    problem: "FLY_CAPABILITY_DENIED",
    revision: "rev-1",
    projectHash: "hash-1",
    draft: { token: "draft-1", generation: 1 },
  });
  expect(state.problems).toEqual([
    expect.objectContaining({ code: "FLY_CAPABILITY_DENIED" }),
  ]);
});

test("unmount invalidates an in-flight success", async ({ page }) => {
  await mountOrderContract(page);
  await beginSaves(page, 1);
  await page.evaluate(() => {
    (globalThis as OrderScope).FlyBrowser?.unmountAll?.();
  });
  await resolveFetch(page, 0, success);
  await awaitSave(page, 0);

  const state = await readState(page);
  expect(state).toMatchObject({
    mounted: "false",
    problem: null,
    revision: "rev-1",
    projectHash: "hash-1",
    draft: { token: "draft-1", generation: 1 },
    problems: [],
  });
  expect(state.responses).toEqual([
    expect.objectContaining({
      status: 200,
      requestGeneration: 1,
      current: false,
    }),
  ]);
});
