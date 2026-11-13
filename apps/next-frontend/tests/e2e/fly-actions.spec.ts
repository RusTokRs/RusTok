import { expect, test, type Page } from '@playwright/test';
import { readFile } from 'node:fs/promises';

// Execute the shipped asset, not a test double or a TypeScript reimplementation. The markup
// mirrors the attributes and style hooks emitted by Fly's built-in blocks and publish renderer.
const runtimePath = new URL('../../../../crates/ui/fly/browser/assets/fly-actions.js', import.meta.url);

function action(value: Record<string, unknown>): string {
  return JSON.stringify(value).replaceAll('&', '&amp;').replaceAll("'", '&#39;');
}

async function mount(page: Page, body: string, css = ''): Promise<void> {
  const runtime = await readFile(runtimePath, 'utf8');
  await page.setContent(`<!doctype html><html><head><style>${css}</style></head><body>${body}</body></html>`);
  await page.addScriptTag({ content: runtime });
}

function tabs(name: string): string {
  return `<div data-testid="${name}">
    <div role="tablist" data-testid="${name}-list">
      <button role="tab" aria-selected="true" data-testid="${name}-tab-1"
        data-fly-action='${action({ kind: 'switch_tab', panel_id: 'tab-panel-1' })}'>First</button>
      <button role="tab" aria-selected="false" data-testid="${name}-tab-2"
        data-fly-action='${action({ kind: 'switch_tab', panel_id: 'tab-panel-2' })}'>Second</button>
    </div>
    <div id="tab-panel-1" role="tabpanel" data-fly-style-id="${name}-panel-1" data-testid="${name}-panel-1">One</div>
    <div id="tab-panel-2" role="tabpanel" hidden aria-hidden="true" data-fly-style-id="${name}-panel-2" data-testid="${name}-panel-2">Two</div>
    <p data-testid="${name}-other">Unrelated content</p>
  </div>`;
}

function accordion(name: string): string {
  return `<div data-fly-accordion="true" data-testid="${name}">
    <button aria-expanded="true" data-testid="${name}-first"
      data-fly-action='${action({ kind: 'toggle_accordion', target_id: `${name}-panel-1`, exclusive: true })}'>First</button>
    <div id="${name}-panel-1" data-testid="${name}-panel-1">One</div>
    <button aria-expanded="false" data-testid="${name}-second"
      data-fly-action='${action({ kind: 'toggle_accordion', target_id: `${name}-panel-2`, exclusive: true })}'>Second</button>
    <div id="${name}-panel-2" hidden data-fly-style-id="${name}-panel-2" data-testid="${name}-panel-2">Two</div>
    <button aria-expanded="true" data-testid="${name}-other"
      data-fly-action='${action({ kind: 'toggle_visibility', target_id: `${name}-extra` })}'>Other control</button>
    <div id="${name}-extra">Extra</div>
    <div data-fly-accordion="true">
      <button aria-expanded="true" data-testid="${name}-nested"
        data-fly-action='${action({ kind: 'toggle_accordion', target_id: `${name}-nested-panel`, exclusive: true })}'>Nested</button>
      <div id="${name}-nested-panel">Nested content</div>
    </div>
  </div>`;
}

function slider(name: string): string {
  return `<div id="${name}" data-fly-slider="true" aria-roledescription="carousel" data-testid="${name}">
    <div data-fly-slider-track="true">
      <div data-fly-slide="0" data-fly-style-id="${name}-slide-1" class="is-active" data-testid="${name}-slide-1">One</div>
      <div data-fly-slide="1" data-fly-style-id="${name}-slide-2" hidden aria-hidden="true" data-testid="${name}-slide-2">Two</div>
    </div>
    <button data-testid="${name}-next" data-fly-action-target="self"
      data-fly-action='${action({ kind: 'switch_slide', slider_id: 'self', direction: 'next' })}'>Next</button>
    <button data-testid="${name}-dot-1"
      data-fly-action='${action({ kind: 'switch_slide', slider_id: 'self', index: 0 })}'>1</button>
    <button data-testid="${name}-dot-2"
      data-fly-action='${action({ kind: 'switch_slide', slider_id: 'self', index: 1 })}'>2</button>
  </div>`;
}

test('tabs switch only their own panels without hiding the tablist or the host', async ({ page }) => {
  await mount(page, `<nav id="tab-panel-2" data-testid="host">Host navigation</nav>
    <main data-fly-page-body="true">${tabs('first')}${tabs('second')}</main>`, `
      [data-fly-style-id$="panel-2"] { display: none; }
    `);

  await page.getByTestId('second-tab-2').click();
  await expect(page.getByTestId('host')).toBeVisible();
  await expect(page.getByTestId('second-list')).toBeVisible();
  await expect(page.getByTestId('second-other')).toBeVisible();
  await expect(page.getByTestId('first-panel-1')).toBeVisible();
  await expect(page.getByTestId('first-panel-2')).toBeHidden();
  await expect(page.getByTestId('second-panel-1')).toBeHidden();
  await expect(page.getByTestId('second-panel-2')).toBeVisible();
  await expect(page.getByTestId('second-tab-2')).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByTestId('second-tab-1')).toHaveAttribute('aria-selected', 'false');
  await page.getByTestId('second-tab-1').click();
  await expect(page.getByTestId('second-panel-1')).toBeVisible();
  await expect(page.getByTestId('second-panel-2')).toBeHidden();
});

test('tabs with a nested header find their panels without affecting another group', async ({ page }) => {
  const nestedTabs = (name: string) => `<section data-testid="${name}">
    <header><div role="tablist">
      <button role="tab" aria-selected="true" data-testid="${name}-tab-1"
        data-fly-action='${action({ kind: 'switch_tab', panel_id: 'shared-panel-1' })}'>First</button>
      <button role="tab" aria-selected="false" data-testid="${name}-tab-2"
        data-fly-action='${action({ kind: 'switch_tab', panel_id: 'shared-panel-2' })}'>Second</button>
    </div></header>
    <div><div id="shared-panel-1" role="tabpanel" data-testid="${name}-panel-1">One</div>
      <div id="shared-panel-2" role="tabpanel" hidden data-testid="${name}-panel-2">Two</div></div>
  </section>`;
  await mount(page, `<nav id="shared-panel-2" data-testid="host">Host</nav>
    <main data-fly-page-body>${nestedTabs('first')}${nestedTabs('second')}</main>`);

  await page.getByTestId('second-tab-2').click();
  await expect(page.getByTestId('second-tab-1')).toHaveAttribute('aria-selected', 'false');
  await expect(page.getByTestId('second-tab-2')).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByTestId('second-panel-1')).toBeHidden();
  await expect(page.getByTestId('second-panel-2')).toBeVisible();
  await expect(page.getByTestId('first-panel-1')).toBeVisible();
  await expect(page.getByTestId('first-panel-2')).toBeHidden();
  await expect(page.getByTestId('host')).toBeVisible();

  await page.getByTestId('second-tab-1').click();
  await expect(page.getByTestId('second-panel-1')).toBeVisible();
  await expect(page.getByTestId('second-panel-2')).toBeHidden();
});

test('a missing panel cannot be borrowed from a neighboring tab group', async ({ page }) => {
  await mount(page, `<main data-fly-page-body>
    <section><header><div role="tablist">
      <button role="tab" aria-selected="true" data-testid="own-first"
        data-fly-action='${action({ kind: 'switch_tab', panel_id: 'own-panel' })}'>First</button>
      <button role="tab" aria-selected="false" data-testid="own-missing"
        data-fly-action='${action({ kind: 'switch_tab', panel_id: 'foreign-panel' })}'>Missing</button>
    </div></header><div id="own-panel" role="tabpanel" data-testid="own-panel">Own panel</div></section>
    <section><div role="tablist"><button role="tab" aria-selected="true" data-testid="foreign-tab"
      data-fly-action='${action({ kind: 'switch_tab', panel_id: 'foreign-panel' })}'>Foreign</button></div>
      <div id="foreign-panel" role="tabpanel" data-testid="foreign-panel">Foreign panel</div></section>
  </main>`);
  await page.getByTestId('own-missing').click();
  await expect(page.getByTestId('own-first')).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByTestId('own-missing')).toHaveAttribute('aria-selected', 'false');
  await expect(page.getByTestId('own-panel')).toBeVisible();
  await expect(page.getByTestId('foreign-tab')).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByTestId('foreign-panel')).toBeVisible();
});

test('nested tab groups switch independently and cannot lend a missing panel to their parent', async ({ page }) => {
  await mount(page, `<main data-fly-page-body><section>
    <header><div role="tablist">
      <button role="tab" aria-selected="true" data-testid="outer-first"
        data-fly-action='${action({ kind: 'switch_tab', panel_id: 'outer-one' })}'>Outer one</button>
      <button role="tab" aria-selected="false" data-testid="outer-second"
        data-fly-action='${action({ kind: 'switch_tab', panel_id: 'outer-two' })}'>Outer two</button>
      <button role="tab" aria-selected="false" data-testid="outer-missing"
        data-fly-action='${action({ kind: 'switch_tab', panel_id: 'inner-two' })}'>Missing</button>
    </div></header>
    <div id="outer-one" data-testid="outer-panel-1">
      <section><header><div role="tablist">
        <button role="tab" aria-selected="true" data-testid="inner-first"
          data-fly-action='${action({ kind: 'switch_tab', panel_id: 'inner-one' })}'>Inner one</button>
        <button role="tab" aria-selected="false" data-testid="inner-second"
          data-fly-action='${action({ kind: 'switch_tab', panel_id: 'inner-two' })}'>Inner two</button>
      </div></header>
        <div id="inner-one" role="tabpanel" data-testid="inner-panel-1">One</div>
        <div id="inner-two" role="tabpanel" hidden data-testid="inner-panel-2">Two</div>
      </section>
    </div>
    <div id="outer-two" hidden data-testid="outer-panel-2">Outer two</div>
  </section></main>`);

  await page.getByTestId('outer-missing').click();
  await expect(page.getByTestId('outer-first')).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByTestId('outer-missing')).toHaveAttribute('aria-selected', 'false');
  await expect(page.getByTestId('inner-panel-2')).toBeHidden();

  await page.getByTestId('inner-second').click();
  await expect(page.getByTestId('inner-panel-1')).toBeHidden();
  await expect(page.getByTestId('inner-panel-2')).toBeVisible();
  await expect(page.getByTestId('outer-first')).toHaveAttribute('aria-selected', 'true');
  await page.getByTestId('outer-second').click();
  await expect(page.getByTestId('outer-panel-1')).toBeHidden();
  await expect(page.getByTestId('outer-panel-2')).toBeVisible();
  await expect(page.getByTestId('inner-first')).toHaveAttribute('aria-selected', 'false');
  await expect(page.getByTestId('inner-second')).toHaveAttribute('aria-selected', 'true');
  await page.getByTestId('outer-first').click();
  await expect(page.getByTestId('inner-panel-2')).toBeVisible();
});

test('a group with no surviving panel cannot borrow from unmarked neighboring controls', async ({ page }) => {
  await mount(page, `<main data-fly-page-body>
    <section><header><div role="tablist"><button role="tab" aria-selected="false" data-testid="missing"
      data-fly-action='${action({ kind: 'switch_tab', panel_id: 'neighbor-panel' })}'>Missing</button></div></header></section>
    <section><div><button aria-selected="true" data-testid="neighbor-tab"
      data-fly-action='${action({ kind: 'switch_tab', panel_id: 'neighbor-panel' })}'>Neighbor</button></div>
      <div id="neighbor-panel" hidden data-testid="neighbor-panel">Neighbor panel</div></section>
  </main>`);
  await page.getByTestId('missing').click();
  await expect(page.getByTestId('missing')).toHaveAttribute('aria-selected', 'false');
  await expect(page.getByTestId('neighbor-tab')).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByTestId('neighbor-panel')).toBeHidden();
});

test('an explicit tab group cannot target an unrelated element beyond its boundary', async ({ page }) => {
  await mount(page, `<main data-fly-page-body><section data-fly-tabs><header><div role="tablist">
    <button role="tab" aria-selected="false" data-testid="missing"
      data-fly-action='${action({ kind: 'switch_tab', panel_id: 'outside' })}'>Missing</button>
  </div></header></section><div id="outside" hidden data-testid="outside">Unrelated</div></main>`);
  await page.getByTestId('missing').click();
  await expect(page.getByTestId('missing')).toHaveAttribute('aria-selected', 'false');
  await expect(page.getByTestId('outside')).toBeHidden();
});

test('exclusive accordions do not alter nested accordions or other expanded controls', async ({ page }) => {
  await mount(page, `<main data-fly-page-body="true">${accordion('faq')}</main>`, `
    [data-fly-style-id="faq-panel-2"] { display: none; }
  `);
  await page.getByTestId('faq-second').click();
  await expect(page.getByTestId('faq-first')).toHaveAttribute('aria-expanded', 'false');
  await expect(page.getByTestId('faq-panel-1')).toBeHidden();
  await expect(page.getByTestId('faq-second')).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByTestId('faq-panel-2')).toBeVisible();
  await expect(page.getByTestId('faq-other')).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByTestId('faq-nested')).toHaveAttribute('aria-expanded', 'true');
});

test('self-referencing sliders ignore a host id and restore the authored display mode', async ({ page }) => {
  await mount(page, `<div id="self" data-testid="host">Host</div>
    <main data-fly-page-body="true">${slider('carousel')}</main>`, `
      [data-fly-style-id="carousel-slide-1"] { display: flex; }
      [data-fly-style-id="carousel-slide-2"] { display: none; }
      [data-testid="carousel-dot-1"] { background: rgb(255, 128, 0); }
      [data-testid="carousel-dot-2"] { background: rgb(128, 0, 255); }
    `);
  await page.getByTestId('carousel-next').click();
  await expect(page.getByTestId('host')).toBeVisible();
  await expect(page.getByTestId('carousel-slide-1')).toBeHidden();
  await expect(page.getByTestId('carousel-slide-2')).toBeVisible();
  await expect(page.getByTestId('carousel-slide-2')).toHaveCSS('display', 'flex');
  await expect(page.getByTestId('carousel-dot-2')).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByTestId('carousel-dot-2')).toHaveCSS('background-color', 'rgb(255, 128, 0)');
  await expect(page.getByTestId('carousel-dot-1')).toHaveCSS('background-color', 'rgb(128, 0, 255)');
  await page.getByTestId('carousel-dot-1').click();
  await expect(page.getByTestId('carousel-slide-1')).toBeVisible();
  await expect(page.getByTestId('carousel-slide-2')).toBeHidden();
});

test('an action cannot select a target in the host or another landing', async ({ page }) => {
  await mount(page, `<header id="host-header" data-testid="host">Host</header>
    <main data-fly-page-body="true">
      <button data-testid="target-host" data-fly-action='${action({ kind: 'toggle_visibility', target_id: 'host-header', mode: 'hide' })}'>Hide</button>
      <button data-testid="target-other" data-fly-action='${action({ kind: 'toggle_visibility', target_id: 'other-page', mode: 'hide' })}'>Hide other</button>
    </main>
    <main data-fly-page-body="true"><p id="other-page" data-testid="other">Another landing</p></main>`);
  await page.getByTestId('target-host').click();
  await page.getByTestId('target-other').click();
  await expect(page.getByTestId('host')).toBeVisible();
  await expect(page.getByTestId('other')).toBeVisible();
});

test('landing slider clicks and swipes cannot operate an ancestor host carousel', async ({ page }) => {
  await mount(page, `<div data-fly-slider aria-roledescription="carousel">
    <div data-fly-slide class="is-active" data-testid="host-slide-1">Host one</div>
    <div data-fly-slide hidden data-testid="host-slide-2">Host two</div>
    <main data-fly-page-body>
      <button data-testid="landing-next" data-fly-action='${action({ kind: 'switch_slide', slider_id: 'self', direction: 'next' })}'>Next</button>
    </main>
  </div>`);
  await page.getByTestId('landing-next').click();
  await expect(page.getByTestId('host-slide-1')).toBeVisible();
  await expect(page.getByTestId('host-slide-2')).toBeHidden();
  await page.getByTestId('landing-next').evaluate((target) => {
    const start = new Event('touchstart', { bubbles: true });
    Object.defineProperty(start, 'touches', { value: [{ clientX: 120, clientY: 0 }] });
    target.dispatchEvent(start);
    const end = new Event('touchend', { bubbles: true });
    Object.defineProperty(end, 'changedTouches', { value: [{ clientX: 0, clientY: 0 }] });
    target.dispatchEvent(end);
  });
  await expect(page.getByTestId('host-slide-1')).toBeVisible();
  await expect(page.getByTestId('host-slide-2')).toBeHidden();
});

test('a swipe operates its own slider but not after cancellation or reparenting', async ({ page }) => {
  await mount(page, `<main data-fly-page-body data-testid="landing">${slider('swipe')}</main>`);
  const swipe = async (cancel: boolean, reparent: boolean) => {
    await page.getByTestId('swipe').evaluate((target, options) => {
      const start = new Event('touchstart', { bubbles: true });
      Object.defineProperty(start, 'touches', { value: [{ clientX: 120, clientY: 0 }] });
      target.dispatchEvent(start);
      if (options.cancel) target.dispatchEvent(new Event('touchcancel', { bubbles: true }));
      if (options.reparent) document.body.appendChild(target);
      const end = new Event('touchend', { bubbles: true });
      Object.defineProperty(end, 'changedTouches', { value: [{ clientX: 0, clientY: 0 }] });
      target.dispatchEvent(end);
    }, { cancel, reparent });
  };
  await swipe(false, false);
  await expect(page.getByTestId('swipe-slide-1')).toBeHidden();
  await expect(page.getByTestId('swipe-slide-2')).toBeVisible();
  await swipe(true, false);
  await expect(page.getByTestId('swipe-slide-2')).toBeVisible();
  await swipe(false, true);
  await expect(page.getByTestId('swipe-slide-2')).toBeVisible();
});

test('landing accordion and tab actions cannot change ancestor host groups', async ({ page }) => {
  await mount(page, `<div data-fly-accordion>
    <button aria-expanded="true" data-testid="host-accordion" data-fly-action='${action({ kind: 'toggle_accordion', target_id: 'host-accordion-panel', exclusive: true })}'>Host</button>
    <div id="host-accordion-panel" data-testid="host-accordion-panel">Host panel</div>
    <main data-fly-page-body>
      <button aria-expanded="false" data-testid="landing-accordion" data-fly-action='${action({ kind: 'toggle_accordion', target_id: 'landing-accordion-panel', exclusive: true })}'>Landing</button>
      <div id="landing-accordion-panel" hidden data-testid="landing-accordion-panel">Landing panel</div>
    </main>
  </div>
  <div><div role="tablist">
    <button role="tab" aria-selected="true" data-testid="host-tab" data-fly-action='${action({ kind: 'switch_tab', panel_id: 'host-tab-panel' })}'>Host</button>
    <main data-fly-page-body>
      <button role="tab" data-testid="landing-tab" data-fly-action='${action({ kind: 'switch_tab', panel_id: 'landing-tab-panel' })}'>Landing</button>
      <div id="landing-tab-panel" hidden data-testid="landing-tab-panel">Landing panel</div>
    </main>
  </div><div id="host-tab-panel" data-testid="host-tab-panel">Host panel</div></div>`);
  await page.getByTestId('landing-accordion').click();
  await expect(page.getByTestId('landing-accordion-panel')).toBeVisible();
  await expect(page.getByTestId('host-accordion')).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByTestId('host-accordion-panel')).toBeVisible();
  await page.getByTestId('landing-tab').click();
  await expect(page.getByTestId('landing-tab-panel')).toBeVisible();
  await expect(page.getByTestId('host-tab')).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByTestId('host-tab-panel')).toBeVisible();
});

test('outer landing groups leave nested landing controls and panels alone', async ({ page }) => {
  await mount(page, `<main data-fly-page-body>
    <div data-fly-accordion>
      <button aria-expanded="false" data-testid="outer-accordion" data-fly-action='${action({ kind: 'toggle_accordion', target_id: 'outer-accordion-panel', exclusive: true })}'>Outer</button>
      <div id="outer-accordion-panel" hidden>Outer panel</div>
      <section data-fly-page-body>
        <button aria-expanded="true" data-testid="inner-accordion" data-fly-action='${action({ kind: 'toggle_accordion', target_id: 'inner-accordion-panel', exclusive: true })}'>Inner</button>
        <div id="inner-accordion-panel" data-testid="inner-accordion-panel">Inner panel</div>
      </section>
    </div>
    <div data-fly-slider>
      <div data-fly-slide class="is-active" data-testid="outer-slide-1">Outer one</div>
      <div data-fly-slide hidden data-testid="outer-slide-2">Outer two</div>
      <section data-fly-page-body><div data-fly-slide data-testid="inner-slide">Inner slide</div></section>
      <button data-testid="outer-next" data-fly-action='${action({ kind: 'switch_slide', slider_id: 'self', direction: 'next' })}'>Next</button>
    </div>
    <div><div role="tablist">
      <button role="tab" aria-selected="false" data-testid="outer-tab" data-fly-action='${action({ kind: 'switch_tab', panel_id: 'outer-tab-panel' })}'>Outer</button>
      <section data-fly-page-body>
        <button role="tab" aria-selected="true" data-testid="inner-tab" data-fly-action='${action({ kind: 'switch_tab', panel_id: 'inner-tab-panel' })}'>Inner</button>
        <div id="inner-tab-panel" data-testid="inner-tab-panel">Inner panel</div>
      </section>
    </div><div id="outer-tab-panel" hidden data-testid="outer-tab-panel">Outer panel</div></div>
  </main>`);
  await page.getByTestId('outer-accordion').click();
  await expect(page.getByTestId('inner-accordion')).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByTestId('inner-accordion-panel')).toBeVisible();
  await page.getByTestId('outer-next').click();
  await expect(page.getByTestId('outer-slide-1')).toBeHidden();
  await expect(page.getByTestId('outer-slide-2')).toBeVisible();
  await expect(page.getByTestId('inner-slide')).toBeVisible();
  await page.getByTestId('outer-tab').click();
  await expect(page.getByTestId('outer-tab-panel')).toBeVisible();
  await expect(page.getByTestId('inner-tab')).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByTestId('inner-tab-panel')).toBeVisible();
});

test('a click in a nested landing does not inherit an ancestor action', async ({ page }) => {
  await mount(page, `<div data-fly-action='${action({ kind: 'toggle_visibility', target_id: 'host-panel', mode: 'hide' })}'>
    <main data-fly-page-body><button data-testid="landing-click">Click in landing</button></main>
  </div><div id="host-panel" data-testid="host-panel">Host panel</div>`);
  await page.getByTestId('landing-click').click();
  await expect(page.getByTestId('host-panel')).toBeVisible();
});

test('component targets resolve with an authored HTML id, with or without style hooks', async ({ page }) => {
  await mount(page, `<main data-fly-page-body="true">
    <button data-testid="toggle" data-fly-action='${action({ kind: 'toggle_visibility', target_id: 'component-1', mode: 'hide' })}'>Hide</button>
    <div id="authored-id" data-fly-target-id="component-1" data-testid="target">Target</div>
    <button data-testid="legacy-toggle" data-fly-action='${action({ kind: 'toggle_visibility', target_id: 'component-2', mode: 'hide' })}'>Hide legacy</button>
    <div id="authored-id-2" data-fly-style-id="component-2" data-testid="legacy-target">Legacy target</div>
  </main>`);
  await page.getByTestId('toggle').click();
  await page.getByTestId('legacy-toggle').click();
  await expect(page.getByTestId('target')).toBeHidden();
  await expect(page.getByTestId('legacy-target')).toBeHidden();
});

test('component targets win over an unrelated authored HTML id', async ({ page }) => {
  await mount(page, `<main data-fly-page-body>
    <div id="panel" data-testid="unrelated"><button data-testid="toggle"
      data-fly-action='${action({ kind: 'toggle_visibility', target_id: 'panel', mode: 'hide' })}'>Hide</button></div>
    <div id="authored-panel" data-fly-target-id="panel" data-testid="component">Actual component</div>
  </main>`);
  await page.getByTestId('toggle').click();
  await expect(page.getByTestId('unrelated')).toBeVisible();
  await expect(page.getByTestId('component')).toBeHidden();
});

test('slider target_slide prefers a component hook over a competing HTML id', async ({ page }) => {
  await mount(page, `<main data-fly-page-body><div data-fly-slider>
    <div data-fly-slide class="is-active" id="target" data-testid="first">First</div>
    <div data-fly-slide hidden data-fly-target-id="target" data-testid="second">Second</div>
    <button data-testid="go" data-fly-action='${action({ kind: 'switch_slide', slider_id: 'self', target_slide: 'target' })}'>Go</button>
  </div></main>`);
  await page.getByTestId('go').click();
  await expect(page.getByTestId('first')).toBeHidden();
  await expect(page.getByTestId('second')).toBeVisible();
});

test('missing targets leave tab and accordion state unchanged', async ({ page }) => {
  await mount(page, `<main data-fly-page-body>${tabs('tabs')}${accordion('faq')}</main>`);
  await page.getByTestId('tabs-tab-2').evaluate((el) => el.setAttribute('data-fly-action', JSON.stringify({ kind: 'switch_tab', panel_id: 'missing' })));
  await page.getByTestId('faq-second').evaluate((el) => el.setAttribute('data-fly-action', JSON.stringify({ kind: 'toggle_accordion', target_id: 'missing', exclusive: true })));
  await page.getByTestId('tabs-tab-2').click();
  await page.getByTestId('faq-second').click();
  await expect(page.getByTestId('tabs-tab-1')).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByTestId('tabs-tab-2')).toHaveAttribute('aria-selected', 'false');
  await expect(page.getByTestId('tabs-panel-1')).toBeVisible();
  await expect(page.getByTestId('tabs-panel-2')).toBeHidden();
  await expect(page.getByTestId('faq-first')).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByTestId('faq-second')).toHaveAttribute('aria-expanded', 'false');
  await expect(page.getByTestId('faq-panel-1')).toBeVisible();
});

test('toggles honor stylesheet and inline display rules across repeated transitions', async ({ page }) => {
  await mount(page, `<main data-fly-page-body>
    <button data-testid="toggle" data-fly-action='${action({ kind: 'toggle_visibility', target_id: 'target' })}'>Toggle</button>
    <div id="target" data-testid="target" style="display: flex"><span>Content</span></div>
    <button data-testid="show" data-fly-action='${action({ kind: 'toggle_visibility', target_id: 'hidden' })}'>Show</button>
    <div id="hidden" data-testid="hidden">Initially stylesheet-hidden</div>
  </main>`, '#target { display: flex !important; } #hidden { display: none !important; }');
  await page.getByTestId('toggle').click();
  await expect(page.getByTestId('target')).toBeHidden();
  await page.getByTestId('toggle').click();
  await expect(page.getByTestId('target')).toBeVisible();
  await expect(page.getByTestId('target')).toHaveCSS('display', 'flex');
  await page.getByTestId('show').click();
  await expect(page.getByTestId('hidden')).toBeVisible();
  await page.getByTestId('show').click();
  await expect(page.getByTestId('hidden')).toBeHidden();
});

test('nested sliders and explicit missing slider targets do not modify another carousel', async ({ page }) => {
  const outer = slider('outer').replace('One</div>', `One ${slider('inner')}</div>`);
  await mount(page, `<main data-fly-page-body>${outer}
    <button data-testid="missing-slider" data-fly-action='${action({ kind: 'switch_slide', slider_id: 'does-not-exist', direction: 'next' })}'>Missing</button>
  </main>`, `[data-fly-style-id$="slide-2"] { display: none; }`);
  await page.getByTestId('outer-next').click();
  await expect(page.getByTestId('outer-slide-1')).toBeHidden();
  await expect(page.getByTestId('outer-slide-2')).toBeVisible();
  await expect(page.getByTestId('inner')).toBeHidden(); // Its parent slide, not its own state.
  await expect(page.getByTestId('inner-slide-1')).not.toHaveAttribute('hidden', '');
  await expect(page.getByTestId('inner-slide-2')).toHaveAttribute('hidden', '');
  await page.getByTestId('missing-slider').click();
  await expect(page.getByTestId('outer-slide-2')).toBeVisible();
  await page.getByTestId('outer-dot-1').click();
  await page.getByTestId('inner-next').click();
  await expect(page.getByTestId('inner-slide-1')).toBeHidden();
  await expect(page.getByTestId('inner-slide-2')).toBeVisible();
  await expect(page.getByTestId('outer-slide-1')).toBeVisible();
  await expect(page.getByTestId('outer-slide-2')).toBeHidden();
});

test('the runtime installs once and stop/start restores event handling', async ({ page }) => {
  await mount(page, `<main data-fly-page-body>
    <button data-testid="toggle" data-fly-action='${action({ kind: 'toggle_visibility', target_id: 'target' })}'>Toggle</button>
    <div id="target" data-testid="target">Target</div>
  </main>`);
  await page.addScriptTag({ content: await readFile(runtimePath, 'utf8') });
  await page.getByTestId('toggle').click();
  await expect(page.getByTestId('target')).toBeHidden();
  await page.evaluate(() => globalThis.FlyActions!.start().stop());
  await page.getByTestId('toggle').click();
  await expect(page.getByTestId('target')).toBeHidden();
  await page.evaluate(() => globalThis.FlyActions!.start());
  await page.getByTestId('toggle').click();
  await expect(page.getByTestId('target')).toBeVisible();
});

test('custom actions do not accidentally submit or reset a form', async ({ page }) => {
  await mount(page, `<main data-fly-page-body><form data-testid="form">
    <input data-testid="text" value="initial">
    <button data-testid="default-submit" data-fly-action='${action({ kind: 'emit_event', event: 'open_cart' })}'>Cart</button>
    <input type="reset" data-testid="reset-action" data-fly-action='${action({ kind: 'emit_event', event: 'open_cart' })}' value="Cart">
    <input type="checkbox" data-testid="checkbox-action" data-fly-action='${action({ kind: 'toggle_visibility', target_id: 'target' })}'>
    <div id="target" data-testid="target">Content</div>
  </form></main>`);
  await page.evaluate(() => {
    document.querySelector('form')!.addEventListener('submit', (event) => {
      event.preventDefault();
      document.querySelector('form')!.setAttribute('data-submitted', 'true');
    });
  });
  await page.getByTestId('text').fill('changed');
  await page.getByTestId('default-submit').click();
  await expect(page.getByTestId('form')).not.toHaveAttribute('data-submitted', 'true');
  await page.getByTestId('reset-action').click();
  await expect(page.getByTestId('text')).toHaveValue('changed');
  await page.getByTestId('checkbox-action').check();
  await expect(page.getByTestId('checkbox-action')).toBeChecked();
  await expect(page.getByTestId('target')).toBeHidden();
});

test('custom actions suppress native activation when the control wraps or is inside the action', async ({ page }) => {
  await mount(page, `<main data-fly-page-body><form data-testid="form">
    <input data-testid="text" value="initial">
    <button type="submit"><span data-testid="wrapped-submit" data-fly-action='${action({ kind: 'emit_event', event: 'open_cart' })}'>Cart</span></button>
    <button type="reset"><span data-testid="wrapped-reset" data-fly-action='${action({ kind: 'emit_event', event: 'open_cart' })}'>Cart</span></button>
    <div data-fly-action='${action({ kind: 'emit_event', event: 'open_cart' })}'><input type="submit" data-testid="nested-submit" value="Cart"></div>
    <a href="#destination"><span data-testid="wrapped-link" data-fly-action='${action({ kind: 'emit_event', event: 'open_cart' })}'>Cart</span></a>
  </form><div id="destination">Destination</div></main>`);
  await page.evaluate(() => {
    document.querySelector('form')!.addEventListener('submit', (event) => {
      event.preventDefault();
      document.querySelector('form')!.setAttribute('data-submitted', 'true');
    });
  });
  await page.getByTestId('text').fill('changed');
  await page.getByTestId('wrapped-submit').click();
  await page.getByTestId('nested-submit').click();
  await expect(page.getByTestId('form')).not.toHaveAttribute('data-submitted', 'true');
  await page.getByTestId('wrapped-reset').click();
  await expect(page.getByTestId('text')).toHaveValue('changed');
  await page.getByTestId('wrapped-link').click();
  expect(await page.evaluate(() => window.location.hash)).toBe('');
});
