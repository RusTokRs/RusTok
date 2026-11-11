import { expect, Page, test } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

// Browser side of the classic-SSR control contract of the Fly editor.
//
// A server-rendered Page Builder has no Leptos event handlers: the palette and the layer tree
// work only because `fly-browser.js` binds the `data-fly-*` attributes that `fly-leptos` renders
// (`FlyPalette`, `FlyLayerTree` in `crates/ui/fly/leptos/src/editor.rs`) and that the Page
// Builder toolbar renders (`AuthoringToolbar`, viewport select). The markup below has the shape
// those components emit; their render tests pin that shape, this file pins what the adapter does
// with it, and `scripts/verify/verify-fly-ssr-control-contract.mjs` keeps the three in step.

const adapterPath = resolve(
  process.cwd(),
  '../../crates/ui/fly/browser/assets/fly-browser.js'
);

type RecordedRequest = {
  input: string;
  method: string;
  body: {
    intent?: string;
    page_id?: string;
    revision?: string;
    payload?: Record<string, unknown>;
  } | null;
};

type AdapterState = {
  selectedId: string | null;
  activeDrag: Record<string, unknown> | null;
  dragging: string | null;
};

type FlyGlobal = typeof globalThis & {
  FlyBrowser?: {
    mount?: (root: Element) => {
      selectedId?: string | null;
      activeDrag?: Record<string, unknown> | null;
    } | null;
  };
  __flyRequests?: RecordedRequest[];
  __flySelections?: unknown[];
};

async function mountSsrControls(page: Page) {
  const adapterSource = await readFile(adapterPath, 'utf8');
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
      <label>
        <span>Device</span>
        <select
          id="viewport"
          data-fly-action="set-viewport"
          data-fly-route-query="fly_viewport"
        >
          <option value="" selected>Custom 1280 × 720</option>
          <option value="desktop-wide">Desktop 1440</option>
          <option value="tablet">Tablet 768</option>
        </select>
      </label>
      <div class="fly-palette" role="group" aria-label="Blocks">
        <details class="fly-palette__category" open>
          <summary>forum</summary>
          <div role="list">
            <div
              id="allowed-item"
              class="fly-palette__item"
              role="listitem"
              data-fly-block-id="forum.topic_list"
              data-fly-category="forum"
              data-fly-contribution-ids="forum.widgets"
              data-fly-can-insert="true"
              data-fly-can-drag="true"
            >
              <button
                id="allowed-insert"
                type="button"
                class="fly-palette__block"
                aria-label="Add: Topic list"
                data-fly-action="insert-block"
              >Topic list</button>
              <button
                id="allowed-drag"
                type="button"
                class="fly-palette__drag"
                aria-label="Drag: Topic list"
                data-fly-action="begin-block-drag"
              >Drag</button>
            </div>
            <div
              id="denied-item"
              class="fly-palette__item"
              role="listitem"
              data-fly-block-id="forum.category_list"
              data-fly-category="forum"
              data-fly-contribution-ids="forum.widgets"
              data-fly-can-insert="false"
              data-fly-can-drag="false"
            >
              <button
                id="denied-insert"
                type="button"
                class="fly-palette__block"
                aria-label="Add: Category list"
                data-fly-action="insert-block"
                disabled
              >Category list</button>
              <button
                id="denied-drag"
                type="button"
                class="fly-palette__drag"
                aria-label="Drag: Category list"
                data-fly-action="begin-block-drag"
                disabled
              >Drag</button>
            </div>
          </div>
        </details>
      </div>
      <ol class="fly-layers" aria-label="Layers">
        <li>
          <button
            id="layer-hero"
            type="button"
            class="fly-layers__row pl-2"
            data-fly-component-id="hero-title"
            data-fly-action="select-component"
          >h1 · hero-title</button>
        </li>
        <li>
          <button id="layer-opaque" type="button" class="fly-layers__row pl-2" disabled>
            Opaque component (preserved)
          </button>
        </li>
      </ol>
      <iframe
        id="canvas-a-frame"
        data-fly-iframe-canvas
        srcdoc="<!doctype html><html><body>canvas</body></html>"
      ></iframe>
    </div>
  `);

  await page.evaluate(async (source) => {
    const root = document.querySelector('#fly-root');
    if (!(root instanceof HTMLElement)) throw new Error('Fly root unavailable');

    const fly = globalThis as FlyGlobal;
    const requests: RecordedRequest[] = [];
    fly.__flyRequests = requests;
    globalThis.fetch = async (input, init = {}) => {
      requests.push({
        input: String(input),
        method: init.method ?? 'GET',
        body: typeof init.body === 'string' ? JSON.parse(init.body) : null
      });
      // `reload: false` keeps the page: a real endpoint answers a mutating intent with
      // `reload: true`, which would discard the state this test reads back.
      return new Response(
        JSON.stringify({
          result: { revision_id: 'rev-2', project_hash: 'hash-2' },
          reload: false,
          draft_token: 'draft-1',
          draft_generation: 1
        }),
        { status: 200, headers: { 'content-type': 'application/json' } }
      );
    };

    const selections: unknown[] = [];
    fly.__flySelections = selections;
    root.addEventListener('fly:select', (event) => {
      selections.push((event as CustomEvent).detail);
    });

    const url = URL.createObjectURL(
      new Blob([source], { type: 'text/javascript' })
    );
    try {
      await import(url);
    } finally {
      URL.revokeObjectURL(url);
    }
  }, adapterSource);

  await expect(page.locator('#fly-root')).toHaveAttribute(
    'data-fly-browser-mounted',
    'true'
  );
}

async function requests(page: Page): Promise<RecordedRequest[]> {
  return page.evaluate(() => (globalThis as FlyGlobal).__flyRequests ?? []);
}

async function adapterState(page: Page): Promise<AdapterState> {
  return page.evaluate(() => {
    const root = document.querySelector('#fly-root');
    if (!(root instanceof HTMLElement)) throw new Error('Fly root unavailable');
    const adapter = (globalThis as FlyGlobal).FlyBrowser?.mount?.(root);
    if (!adapter) throw new Error('Fly browser adapter unavailable');
    return {
      selectedId: adapter.selectedId ?? null,
      activeDrag: adapter.activeDrag ?? null,
      dragging: root.dataset.flyDragging ?? null
    };
  });
}

test('the palette insert control posts insert_block for its block', async ({
  page
}) => {
  await mountSsrControls(page);

  await page.locator('#allowed-insert').click();

  await expect.poll(async () => (await requests(page)).length).toBe(1);
  const [request] = await requests(page);
  expect(request.input).toContain('/fly-intent');
  expect(request.method).toBe('POST');
  expect(request.body).toMatchObject({
    intent: 'insert_block',
    page_id: 'home',
    revision: 'rev-1',
    payload: { block_id: 'forum.topic_list' }
  });
});

test('a disabled insert control posts nothing', async ({ page }) => {
  await mountSsrControls(page);

  await page.evaluate(() => {
    const button = document.querySelector('#denied-insert');
    if (!(button instanceof HTMLButtonElement))
      throw new Error('insert control unavailable');
    button.click();
  });
  // A click that did reach the adapter would post within the same task.
  await page.evaluate(
    () => new Promise((resolve) => requestAnimationFrame(() => resolve(null)))
  );

  expect(await requests(page)).toEqual([]);
});

test('the palette drag control arms a block drag without a request', async ({
  page
}) => {
  await mountSsrControls(page);

  await page.locator('#allowed-drag').focus();
  await page.keyboard.press('Enter');

  expect(await adapterState(page)).toMatchObject({
    activeDrag: { kind: 'block', block_id: 'forum.topic_list' },
    dragging: 'block'
  });
  expect(await requests(page)).toEqual([]);
});

test('only a palette item that may be dragged becomes a drag source', async ({
  page
}) => {
  await mountSsrControls(page);

  await expect(page.locator('#allowed-item')).toHaveAttribute(
    'draggable',
    'true'
  );
  await expect(page.locator('#denied-item')).not.toHaveAttribute('draggable');

  const dragStart = (selector: string) =>
    page.evaluate((target) => {
      const item = document.querySelector(target);
      if (!(item instanceof HTMLElement))
        throw new Error('palette item unavailable');
      item.dispatchEvent(
        new DragEvent('dragstart', {
          bubbles: true,
          dataTransfer: new DataTransfer()
        })
      );
    }, selector);

  await dragStart('#denied-item');
  expect(await adapterState(page)).toMatchObject({
    activeDrag: null,
    dragging: null
  });

  await dragStart('#allowed-item');
  expect(await adapterState(page)).toMatchObject({
    activeDrag: { kind: 'block', block_id: 'forum.topic_list' },
    dragging: 'block'
  });
});

test('a layer row selects its component and later intents carry the selection', async ({
  page
}) => {
  await mountSsrControls(page);

  await page.locator('#layer-hero').click();

  expect(await adapterState(page)).toMatchObject({ selectedId: 'hero-title' });
  expect(
    await page.evaluate(() => (globalThis as FlyGlobal).__flySelections ?? [])
  ).toEqual([{ componentId: 'hero-title', source: 'ssr-control' }]);
  // Selecting is local: the server learns the selection from the next intent.
  expect(await requests(page)).toEqual([]);

  await page.locator('#allowed-insert').click();
  await expect.poll(async () => (await requests(page)).length).toBe(1);
  const [request] = await requests(page);
  expect(request.body?.payload).toMatchObject({
    block_id: 'forum.topic_list',
    selected_component_id: 'hero-title'
  });
});

test('a row without a component id is not a selection or drag source', async ({
  page
}) => {
  await mountSsrControls(page);

  await expect(page.locator('#layer-hero')).toHaveAttribute(
    'draggable',
    'true'
  );
  await expect(page.locator('#layer-opaque')).not.toHaveAttribute('draggable');
  expect(await adapterState(page)).toMatchObject({ selectedId: null });
});

// The viewport switch is view state the server renders from the request, so the adapter writes
// the chosen preset into the route and navigates instead of posting an intent.
const editorOrigin = 'http://fly-editor.test';

async function openEditorRoute(page: Page, search: string) {
  await page.route(`${editorOrigin}/**`, (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: '<!doctype html><html><body data-fly-navigated></body></html>'
    })
  );
  await page.goto(`${editorOrigin}/admin/pages/home${search}`);
  await mountSsrControls(page);
}

test('the viewport select writes the preset into the route and keeps the rest of it', async ({
  page
}) => {
  await openEditorRoute(page, '?fly_draft=draft-1&tab=builder');

  await page.locator('#viewport').selectOption('tablet');

  await page.waitForURL(
    (url) => url.searchParams.get('fly_viewport') === 'tablet'
  );
  const url = new URL(page.url());
  expect(url.origin + url.pathname).toBe(`${editorOrigin}/admin/pages/home`);
  expect(url.searchParams.get('fly_draft')).toBe('draft-1');
  expect(url.searchParams.get('tab')).toBe('builder');
  // The server answered the navigation: this is a new document, not the mounted one.
  await expect(page.locator('body')).toHaveAttribute('data-fly-navigated', '');
});

test('the viewport select replaces a preset that is already in the route', async ({
  page
}) => {
  await openEditorRoute(page, '?fly_viewport=tablet&fly_draft=draft-1');

  await page.locator('#viewport').selectOption('desktop-wide');

  await page.waitForURL(
    (url) => url.searchParams.get('fly_viewport') === 'desktop-wide'
  );
  const url = new URL(page.url());
  expect(url.searchParams.getAll('fly_viewport')).toEqual(['desktop-wide']);
  expect(url.searchParams.get('fly_draft')).toBe('draft-1');
});

test('the viewport select does not navigate for the preset in effect or for no preset', async ({
  page
}) => {
  await openEditorRoute(page, '?fly_viewport=tablet');
  const before = page.url();

  await page.locator('#viewport').selectOption('tablet');
  await page.locator('#viewport').selectOption('');

  // Neither choice changes what the server would render, so the mounted page stays.
  await expect(page.locator('#fly-root')).toHaveAttribute(
    'data-fly-browser-mounted',
    'true'
  );
  expect(page.url()).toBe(before);
  expect(await requests(page)).toEqual([]);
});

test('the viewport select posts no intent', async ({ page }) => {
  await openEditorRoute(page, '');
  const posted: string[] = [];
  page.on('request', (request) => {
    if (request.method() === 'POST') posted.push(request.url());
  });

  await page.locator('#viewport').selectOption('tablet');
  await page.waitForURL(
    (url) => url.searchParams.get('fly_viewport') === 'tablet'
  );

  expect(posted).toEqual([]);
});
