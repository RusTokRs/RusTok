import { expect, Page, test } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

import { scopeLandingCss } from '../../packages/rustok-pages/src/components/css-scope';

// The package tests check the output of `scopeLandingCss` with a scanner of their own. Here the
// judge is the browser: the stylesheet is emitted as the text of a `<style>` element, the way the
// storefront emits it, and the CSS object model says what the browser made of it.

const parityVectorsPath = resolve(
  process.cwd(),
  '../../crates/modules/rustok-page-builder/contracts/landing-css-scope-parity.json',
);

// The scope roots of the two storefronts: the Next.js container and the Leptos landing body.
const scopeSelectors = ['.fly-page-container', '[data-fly-page-body]'];

// Stylesheets whose block structure is unbalanced or easy to misread.
const hostileStylesheets = [
  '} header { display: none; }',
  '.a { color: red; } } header { display: none; }',
  '.a { color: red;',
  '@media (min-width: 1px) { .a { color: red;',
  '.a { content: "unterminated',
  '.a { color: red; } /* unterminated',
  '.a { background: url(unterminated',
  '.a { width: calc((1px + 2px) ',
  '.a { color: red; } \\',
  '.a::before { content: "}"; } header { display: none; }',
  ".a::before { content: '\\'}'; } .b { color: blue; }",
  '.a { background: url(x}y.png); } .b { color: blue; }',
  '.a { background: url(x{y.png); } } header { display: none; }',
  '.a\\} { color: red; } .b { color: blue; }',
  '/* } */ .a { color: red; }',
  '.a { content: "open\n} } header { display: none; }',
  '.a { width: calc(1px } .b { color: blue; }',
  '}}}} header { display: none; }',
  '@scope (header) { :scope { display: none; } } } header { display: none; }',
  '.a { color: red; } </style><style>header { display: none; }',
  '.a::before { content: "</style><img id=leak src=x>"; }',
  '.a { color: red; } </STYLE ><img id="leak" src="x">',
  '@import url(x.css); header { display: none; }',
  '@media screen { } } header { display: none; } @media screen {',
];

type ParityVectors = { cases: Array<{ name: string; css: string }> };

type StylesheetReport = {
  styleElements: number;
  topLevelRules: string[];
  scopeStart: string | null;
  leakedElements: number;
  strayText: string;
  hostHeaderDisplay: string;
  hostHeaderColor: string;
  endMarkerPresent: boolean;
};

async function renderLanding(page: Page, scopedCss: string): Promise<StylesheetReport> {
  // The markup the storefront produces around a landing, with host elements on both sides.
  await page.setContent(`<!doctype html>
<html>
  <body>
    <header id="host-header" style="color: rgb(10, 20, 30)">Host header</header>
    <main class="fly-page-container" data-fly-page-body="true"><style id="landing">${scopedCss}</style><div class="fly-page-body"><header class="a b">Landing header</header></div></main>
    <footer id="end-marker">Host footer</footer>
  </body>
</html>`);

  return page.evaluate(() => {
    const style = document.querySelector('style#landing') as HTMLStyleElement;
    const rules = Array.from(style.sheet?.cssRules ?? []);
    const first = rules[0] as (CSSRule & { start?: string | null }) | undefined;
    const hostHeader = document.querySelector('#host-header') as HTMLElement;
    const strayText = Array.from(document.querySelectorAll('body, main.fly-page-container'))
      .flatMap((parent) => Array.from(parent.childNodes))
      .filter((node) => node.nodeType === Node.TEXT_NODE)
      .map((node) => node.textContent ?? '')
      .join('')
      .trim();
    return {
      styleElements: document.querySelectorAll('style').length,
      topLevelRules: rules.map((rule) => rule.constructor.name),
      scopeStart: first?.start ?? null,
      leakedElements: document.querySelectorAll('#leak, img, script').length,
      strayText,
      hostHeaderDisplay: getComputedStyle(hostHeader).display,
      hostHeaderColor: getComputedStyle(hostHeader).color,
      endMarkerPresent: document.querySelector('main.fly-page-container + #end-marker') !== null,
    };
  });
}

function expectConfined(report: StylesheetReport, scopeSelector: string, source: string) {
  const label = `stylesheet ${JSON.stringify(source)} scoped to ${scopeSelector}`;
  // The browser reads the whole text as one `@scope` rule: nothing follows the wrapper.
  expect(report.topLevelRules, label).toEqual(['CSSScopeRule']);
  expect(report.scopeStart, label).toBe(scopeSelector);
  // The `<style>` element holds all of it: no markup or text escaped into the document.
  expect(report.styleElements, label).toBe(1);
  expect(report.leakedElements, label).toBe(0);
  expect(report.strayText, label).toBe('');
  expect(report.endMarkerPresent, label).toBe(true);
  // The host element outside the landing keeps its own style.
  expect(report.hostHeaderDisplay, label).toBe('block');
  expect(report.hostHeaderColor, label).toBe('rgb(10, 20, 30)');
}

test('the browser reads every shared parity vector as one @scope rule', async ({ page }) => {
  const vectors = JSON.parse(await readFile(parityVectorsPath, 'utf8')) as ParityVectors;
  expect(vectors.cases.length).toBeGreaterThan(0);

  for (const scopeSelector of scopeSelectors) {
    for (const vector of vectors.cases) {
      const scoped = scopeLandingCss(vector.css, scopeSelector);
      const report = await renderLanding(page, scoped);
      expectConfined(report, scopeSelector, vector.css);
    }
  }
});

test('the browser keeps unbalanced and hostile stylesheets inside the landing', async ({
  page,
}) => {
  for (const scopeSelector of scopeSelectors) {
    for (const css of hostileStylesheets) {
      const scoped = scopeLandingCss(css, scopeSelector);
      const report = await renderLanding(page, scoped);
      expectConfined(report, scopeSelector, css);
    }
  }
});

test('document-level selectors style the landing container, not the host document', async ({
  page,
}) => {
  for (const scopeSelector of scopeSelectors) {
    const scoped = scopeLandingCss(
      [
        'body { background-color: rgb(1, 2, 3); }',
        'html { outline: 4px solid rgb(4, 5, 6); }',
        ':root { --landing-token: 7px; }',
        'header { color: rgb(7, 8, 9); }',
      ].join(' '),
      scopeSelector,
    );
    const report = await renderLanding(page, scoped);
    expectConfined(report, scopeSelector, 'document-level selectors');

    const styles = await page.evaluate(() => {
      const container = document.querySelector('main.fly-page-container') as HTMLElement;
      const landingHeader = container.querySelector('header') as HTMLElement;
      const read = (element: Element) => {
        const style = getComputedStyle(element);
        return {
          backgroundColor: style.backgroundColor,
          outlineWidth: style.outlineWidth,
          token: style.getPropertyValue('--landing-token').trim(),
        };
      };
      return {
        container: read(container),
        body: read(document.body),
        root: read(document.documentElement),
        landingHeaderColor: getComputedStyle(landingHeader).color,
      };
    });

    expect(styles.container).toEqual({
      backgroundColor: 'rgb(1, 2, 3)',
      outlineWidth: '4px',
      token: '7px',
    });
    expect(styles.body.backgroundColor).toBe('rgba(0, 0, 0, 0)');
    expect(styles.root.outlineWidth).not.toBe('4px');
    expect(styles.body.token).toBe('');
    expect(styles.root.token).toBe('');
    expect(styles.landingHeaderColor).toBe('rgb(7, 8, 9)');
  }
});

test('a string that spells </style> stays a CSS string', async ({ page }) => {
  const scoped = scopeLandingCss(
    '.a::before { content: "</style><img id=leak src=x>"; }',
    '.fly-page-container',
  );
  const report = await renderLanding(page, scoped);
  expectConfined(report, '.fly-page-container', 'a </style> string');

  const content = await page.evaluate(() => {
    const landingHeader = document.querySelector('main.fly-page-container header') as HTMLElement;
    return getComputedStyle(landingHeader, '::before').content;
  });
  expect(content).toBe('"</style><img id=leak src=x>"');
});
