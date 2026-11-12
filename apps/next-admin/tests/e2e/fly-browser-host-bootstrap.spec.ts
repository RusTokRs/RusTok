import { expect, Page, test } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

// Browser side of the classic-SSR form contract of the Page Builder.
//
// The SSR panels (`crates/modules/rustok-page-builder/admin/src/editor/ssr_*.rs`) render plain
// forms. They work without hydration only because the host bootstrap
// (`PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS` in
// `crates/modules/rustok-page-builder/src/browser_host.rs`) binds three attributes on top of the
// Fly browser adapter: `form[data-fly-intent-form]`, `[data-fly-component-picker]` and
// `[data-fly-selected-component-input]`. This file loads the module exactly as
// `page_builder_browser_module` composes it — config, shipped adapter, shipped bootstrap — and
// pins what the bootstrap does with markup of the shape those panels emit.
// `scripts/verify/verify-fly-ssr-control-contract.mjs` keeps the views, the bootstrap, the
// server handlers and this file in step.

const adapterPath = resolve(
  process.cwd(),
  '../../crates/ui/fly/browser/assets/fly-browser.js'
);
const hostBootstrapPath = resolve(
  process.cwd(),
  '../../crates/modules/rustok-page-builder/src/browser_host.rs'
);
const hostBootstrapConstant = 'PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS';

type RecordedRequest = {
  input: string;
  method: string;
  body: {
    intent?: string;
    page_id?: string;
    draft_token?: string | null;
    payload?: Record<string, unknown>;
  } | null;
};

type FlyGlobal = typeof globalThis & {
  FlyBrowser?: {
    mount?: (root: Element) => { selectedId?: string | null } | null;
  };
  __flyRequests?: RecordedRequest[];
  __flySelections?: unknown[];
};

/** The module source a server-rendered Page Builder page carries. */
async function hostModuleSource(): Promise<string> {
  const [adapterSource, hostSource] = await Promise.all([
    readFile(adapterPath, 'utf8'),
    readFile(hostBootstrapPath, 'utf8')
  ]);
  // The constant is a Rust raw string, so its text is the JavaScript verbatim.
  const start = hostSource.indexOf(
    `pub const ${hostBootstrapConstant}: &str = r#"`
  );
  const open = hostSource.indexOf('r#"', start) + 3;
  const close = hostSource.indexOf('"#;', open);
  if (start < 0 || close < 0) {
    throw new Error(`${hostBootstrapConstant} was not found`);
  }
  const config = JSON.stringify({
    expectedOrigin: 'null',
    intentEndpoint: '/fly-intent'
  });
  return [
    `globalThis.__FLY_BROWSER_CONFIG__ = Object.freeze(${config});`,
    adapterSource,
    hostSource.slice(open, close)
  ].join('\n');
}

async function importHostModule(page: Page) {
  const source = await hostModuleSource();
  await page.evaluate(async (moduleSource) => {
    const url = URL.createObjectURL(
      new Blob([moduleSource], { type: 'text/javascript' })
    );
    try {
      await import(url);
    } finally {
      URL.revokeObjectURL(url);
    }
  }, source);
}

async function mountHost(
  page: Page,
  options: { reload?: boolean; answer?: Record<string, unknown> } = {}
) {
  await page.setContent(`
    <div
      id="fly-root"
      data-fly-browser-root
      data-fly-page-id="home"
      data-fly-revision="rev-1"
      data-fly-project-hash="hash-1"
      style="position:relative"
    >
      <ol class="fly-layers" aria-label="Layers">
        <li>
          <button
            id="layer-cta"
            type="button"
            class="fly-layers__row pl-2"
            data-fly-component-id="cta"
            data-fly-action="select-component"
          >a · cta</button>
        </li>
      </ol>
      <section data-fly-ssr-inspector="true">
        <select id="picker" data-fly-component-picker="true">
          <option value="hero">h1 (hero)</option>
          <option value="cta">a (cta)</option>
        </select>
        <form id="property-form" data-fly-intent-form="patch_component_property">
          <input
            id="property-component"
            type="hidden"
            name="component_id"
            data-fly-selected-component-input="true"
            value="hero"
          />
          <select name="kind">
            <option value="field">Field</option>
            <option value="attribute">Attribute</option>
          </select>
          <input id="property-name" name="name" required />
          <textarea id="property-value" name="value"></textarea>
          <label>
            <input id="property-remove" type="checkbox" name="remove" value="true" />
            Remove
          </label>
          <button id="property-submit" type="submit">Apply</button>
        </form>
        <form id="open-page-form" data-fly-intent-form="activate_page">
          <select id="open-page-id" name="page_id">
            <option value="home" selected>Home</option>
            <option value="about">About</option>
          </select>
          <button id="open-page-submit" type="submit">Open</button>
        </form>
        <form id="create-page-form" data-fly-intent-form="create_page">
          <input id="create-page-id" name="page_id" required />
          <input id="create-page-name" name="name" />
          <button id="create-page-submit" type="submit">Create</button>
        </form>
        <form id="rename-page-form" data-fly-intent-form="rename_page">
          <select id="rename-page-id" name="page_id">
            <option value="home">Home</option>
            <option value="about">About</option>
          </select>
          <input id="rename-new-page-id" required name="new_page_id" />
          <input id="rename-page-name" name="name" />
          <button id="rename-page-submit" type="submit">Rename</button>
        </form>
        <form id="remove-page-form" data-fly-intent-form="remove_page">
          <select id="remove-page-id" name="page_id">
            <option value="home">Home</option>
            <option value="about">About</option>
          </select>
          <button id="remove-page-submit" type="submit">Remove</button>
        </form>
      </section>
      <section data-fly-ssr-actions-forms="true">
        <form id="field-form" data-fly-intent-form="set_native_form_field">
          <input
            id="field-component"
            type="hidden"
            name="component_id"
            value="hero"
            data-fly-selected-component-input="true"
          />
          <input id="field-name" required name="name" value="email" />
          <label>
            <input id="field-required" type="checkbox" name="required" value="true" checked />
            Required
          </label>
          <input id="field-min-length" type="number" min="0" name="min_length" value="3" />
          <input id="field-max-length" type="number" min="0" name="max_length" value="" />
          <button id="field-submit" type="submit">Save</button>
        </form>
      </section>
      <section
        data-page-builder-server-preview="true"
        data-page-builder-provider-preview="true"
      >
        <form id="preview-form" data-fly-intent-form="render_server_preview">
          <button id="preview-submit" type="submit">Render preview</button>
        </form>
        <iframe
          id="preview-frame"
          data-page-builder-server-preview-frame="true"
          hidden
          sandbox
        ></iframe>
      </section>
      <iframe
        data-fly-iframe-canvas
        srcdoc="<!doctype html><html><body>canvas</body></html>"
      ></iframe>
    </div>
  `);

  await page.evaluate(({ reload, answer }) => {
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
      // `reload: true`, which would discard the state a test reads back. A test of the reload
      // itself asks for it.
      return new Response(
        JSON.stringify({
          result: { revision_id: 'rev-2', project_hash: 'hash-2' },
          reload,
          draft_token: 'draft-1',
          draft_generation: 1,
          ...answer
        }),
        { status: 200, headers: { 'content-type': 'application/json' } }
      );
    };

    const selections: unknown[] = [];
    fly.__flySelections = selections;
    root.addEventListener('fly:select', (event) => {
      selections.push((event as CustomEvent).detail);
    });
  }, { reload: options.reload === true, answer: options.answer ?? {} });

  await importHostModule(page);
  await expect(page.locator('#fly-root')).toHaveAttribute(
    'data-fly-browser-mounted',
    'true'
  );
}

async function requests(page: Page): Promise<RecordedRequest[]> {
  return page.evaluate(() => (globalThis as FlyGlobal).__flyRequests ?? []);
}

async function selectedId(page: Page): Promise<string | null> {
  return page.evaluate(() => {
    const root = document.querySelector('#fly-root');
    if (!(root instanceof HTMLElement)) throw new Error('Fly root unavailable');
    return (
      (globalThis as FlyGlobal).FlyBrowser?.mount?.(root)?.selectedId ?? null
    );
  });
}

/** What a canvas click on empty space leads to: the adapter announces an empty selection. */
async function clearSelection(page: Page) {
  await page.evaluate(() => {
    document.querySelector('#fly-root')?.dispatchEvent(
      new CustomEvent('fly:select', {
        bubbles: true,
        detail: { componentId: null, source: 'iframe' }
      })
    );
  });
}

test('a form with data-fly-intent-form posts its intent instead of submitting', async ({
  page
}) => {
  await mountHost(page);

  await page.locator('#create-page-id').fill('about');
  await page.locator('#create-page-name').fill('About');
  await page.locator('#create-page-submit').click();

  await expect.poll(async () => (await requests(page)).length).toBe(1);
  const [request] = await requests(page);
  expect(request.input).toContain('/fly-intent');
  expect(request.method).toBe('POST');
  expect(request.body).toMatchObject({
    intent: 'create_page',
    page_id: 'home',
    payload: { page_id: 'about', name: 'About' }
  });
});

test('a page form names its page by id and sends no page index', async ({
  page
}) => {
  await mountHost(page);

  await page.locator('#rename-page-id').selectOption('about');
  await page.locator('#rename-new-page-id').fill('company');
  await page.locator('#rename-page-submit').click();
  await page.locator('#remove-page-id').selectOption('about');
  await page.locator('#remove-page-submit').click();
  await page.locator('#open-page-id').selectOption('about');
  await page.locator('#open-page-submit').click();

  await expect.poll(async () => (await requests(page)).length).toBe(3);
  const [rename, remove, open] = await requests(page);
  // The envelope names the page the editor is on, the payload the page the form acts on.
  expect(rename.body).toMatchObject({ intent: 'rename_page', page_id: 'home' });
  // An optional text input left empty is sent as `""`, not left out.
  expect(rename.body?.payload).toMatchObject({
    page_id: 'about',
    new_page_id: 'company',
    name: ''
  });
  expect(remove.body).toMatchObject({ intent: 'remove_page', page_id: 'home' });
  expect(remove.body?.payload).toMatchObject({ page_id: 'about' });
  // Opening a page is the same shape: the page tabs of the workspace need hydration, and
  // this form is how the default runtime changes the page.
  expect(open.body).toMatchObject({ intent: 'activate_page', page_id: 'home' });
  expect(open.body?.payload).toMatchObject({ page_id: 'about' });
  // The server tells a form from a typed envelope of the same intent by these two keys:
  // `page_id` present, `page_index` absent. A form has no index to send.
  for (const request of [rename, remove, open]) {
    expect(request.body?.payload).not.toHaveProperty('page_index');
  }
});

test('a form payload carries booleans for checkboxes and numbers for number inputs', async ({
  page
}) => {
  await mountHost(page);

  await page.locator('#field-submit').click();
  await page.locator('#property-name').fill('title');
  await page.locator('#property-submit').click();

  await expect.poll(async () => (await requests(page)).length).toBe(2);
  const [field, property] = await requests(page);
  expect(field.body?.intent).toBe('set_native_form_field');
  expect(field.body?.payload).toMatchObject({
    name: 'email',
    required: true,
    min_length: 3
  });
  // An empty number input is an absent value, not `""` and not `0`.
  expect(field.body?.payload).not.toHaveProperty('max_length');
  // An unchecked checkbox is `false`; a plain form submission would leave it out.
  expect(property.body?.payload).toMatchObject({
    name: 'title',
    remove: false
  });
});

test('the component the server rendered a form for survives the mount', async ({
  page
}) => {
  await mountHost(page);

  // Nothing is selected in the browser yet, so the server-rendered target stands.
  expect(await selectedId(page)).toBeNull();
  await expect(page.locator('#property-component')).toHaveValue('hero');
  await expect(page.locator('#field-component')).toHaveValue('hero');
  await expect(page.locator('#picker')).toHaveValue('hero');

  await page.locator('#property-name').fill('title');
  await page.locator('#property-submit').click();

  await expect.poll(async () => (await requests(page)).length).toBe(1);
  const [request] = await requests(page);
  expect(request.body?.payload).toMatchObject({ component_id: 'hero' });
});

test('the component picker selects the component and retargets every form', async ({
  page
}) => {
  await mountHost(page);

  await page.locator('#picker').selectOption('cta');

  await expect(page.locator('#property-component')).toHaveValue('cta');
  await expect(page.locator('#field-component')).toHaveValue('cta');
  expect(await selectedId(page)).toBe('cta');
  expect(
    await page.evaluate(() => (globalThis as FlyGlobal).__flySelections)
  ).toEqual([{ componentId: 'cta', source: 'ssr-inspector' }]);

  await page.locator('#field-submit').click();

  await expect.poll(async () => (await requests(page)).length).toBe(1);
  const [request] = await requests(page);
  expect(request.body?.payload).toMatchObject({
    component_id: 'cta',
    selected_component_id: 'cta'
  });
});

test('a selection made outside the picker retargets the forms and the picker', async ({
  page
}) => {
  await mountHost(page);

  await page.locator('#layer-cta').click();

  await expect(page.locator('#property-component')).toHaveValue('cta');
  await expect(page.locator('#field-component')).toHaveValue('cta');
  // The picker names the component the forms act on, whoever selected it.
  await expect(page.locator('#picker')).toHaveValue('cta');
});

test('clearing the selection restores the component the server rendered', async ({
  page
}) => {
  await mountHost(page);
  await page.locator('#picker').selectOption('cta');
  await expect(page.locator('#property-component')).toHaveValue('cta');

  await clearSelection(page);

  await expect(page.locator('#property-component')).toHaveValue('hero');
  await expect(page.locator('#field-component')).toHaveValue('hero');
  await expect(page.locator('#picker')).toHaveValue('hero');
});

test('loading the module again does not bind the forms twice', async ({
  page
}) => {
  await mountHost(page);
  await importHostModule(page);

  await page.locator('#create-page-id').fill('about');
  await page.locator('#create-page-submit').click();

  await expect.poll(async () => (await requests(page)).length).toBe(1);
  await page.waitForTimeout(100);
  expect(await requests(page)).toHaveLength(1);
});

// The draft token is route state: the server reads `?fly_draft=` when it renders, so the
// bootstrap adopts the token of the route and writes the token of a response back into it.
const editorOrigin = 'http://fly-editor.test';

async function openEditorRoute(
  page: Page,
  search: string,
  options: { reload?: boolean } = {}
) {
  await page.route(`${editorOrigin}/**`, (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: '<!doctype html><html><body></body></html>'
    })
  );
  await page.goto(`${editorOrigin}/admin/pages/home${search}`);
  await mountHost(page, options);
}

test('the draft token of the route is sent with the first intent', async ({
  page
}) => {
  await openEditorRoute(page, '?fly_draft=route-draft');

  await page.locator('#create-page-id').fill('about');
  await page.locator('#create-page-submit').click();

  await expect.poll(async () => (await requests(page)).length).toBe(1);
  const [request] = await requests(page);
  expect(request.body?.draft_token).toBe('route-draft');
});

test('a preview answer renders into the panel frame and reveals it', async ({ page }) => {
  // The panel renders the frame before a document exists and hides it; the answer is what fills
  // it. A frame created only after a re-render would be one this binding cannot reach.
  await mountHost(page, {
    answer: {
      persistence: {
        capability: 'preview',
        result: { page_id: 'home', html: '<!doctype html><html><body><p>preview</p></body></html>' }
      }
    }
  });

  const frame = page.locator('#preview-frame');
  await expect(frame).toBeHidden();
  await page.locator('#preview-submit').click();

  await expect
    .poll(async () => frame.getAttribute('srcdoc'))
    .toContain('<p>preview</p>');
  await expect(frame).toBeVisible();
  // The frame is what the answer names, not a second request.
  const posted = await requests(page);
  expect(posted).toHaveLength(1);
  expect(posted[0].body?.intent).toBe('render_server_preview');
});

test('an answer that is not a preview leaves the panel frame alone', async ({ page }) => {
  await mountHost(page);
  await page.locator('#preview-submit').click();
  await expect.poll(async () => (await requests(page)).length).toBe(1);

  const frame = page.locator('#preview-frame');
  expect(await frame.getAttribute('srcdoc')).toBeNull();
  await expect(frame).toBeHidden();
});

test('the draft token of a response is written into the route without a navigation', async ({
  page
}) => {
  await openEditorRoute(page, '?tab=builder');

  await page.locator('#create-page-id').fill('about');
  await page.locator('#create-page-submit').click();

  await page.waitForURL(
    (url) => url.searchParams.get('fly_draft') === 'draft-1'
  );
  const url = new URL(page.url());
  expect(url.searchParams.get('tab')).toBe('builder');
  // `history.replaceState`, not a navigation: the mounted document is still there.
  await expect(page.locator('#fly-root')).toHaveAttribute(
    'data-fly-browser-mounted',
    'true'
  );
});

test('an answer that asks for a reload reloads the route that carries the draft token', async ({
  page
}) => {
  // The page the editor is on lives in the draft session. The server renders it only when
  // the reloaded address names that session, so the token is in the route before the reload.
  await openEditorRoute(page, '?tab=builder', { reload: true });
  const reloaded: string[] = [];
  page.on('framenavigated', (frame) => {
    if (frame === page.mainFrame()) reloaded.push(frame.url());
  });

  await page.locator('#open-page-id').selectOption('about');
  await page.locator('#open-page-submit').click();

  await expect.poll(() => reloaded.length).toBeGreaterThan(0);
  const url = new URL(reloaded[reloaded.length - 1]);
  expect(url.searchParams.get('fly_draft')).toBe('draft-1');
  expect(url.searchParams.get('tab')).toBe('builder');
  // A navigation, not `history.replaceState`: the document the form was in is gone.
  await expect(page.locator('#fly-root')).toHaveCount(0);
});
