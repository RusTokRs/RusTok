import { expect, test, type Page } from '@playwright/test';
import { readFile } from 'node:fs/promises';

// This exercises the shipped browser guard, not a mounted Leptos/WASM owner.
const bootstrap = new URL('../../../../apps/storefront/public/assets/product-buyer-bootstrap.js', import.meta.url);
type Guard = { current(): boolean; replace(href: string): boolean };
type GuardWindow = Window & { installGuard(win: Window): Guard; guard: Guard; invalidations: number };

async function install(page: Page): Promise<void> {
  await page.route('https://route.test/**', route => route.fulfill({
    contentType: 'text/html', body: '<!doctype html><html><body>Storefront</body></html>',
  }));
  await page.goto('https://route.test/en/modules/products?quantity=1#buyer');
  const source = await readFile(bootstrap, 'utf8');
  await page.addScriptTag({ type: 'module', content: `${source}\nwindow.installGuard = installStorefrontNavigationGuard;` });
  await page.waitForFunction(() => typeof (window as unknown as GuardWindow).installGuard === 'function');
  await page.evaluate(() => {
    const state = window as unknown as GuardWindow;
    state.invalidations = 0;
    state.addEventListener('rustok:storefront:navigation', () => state.invalidations++);
    state.guard = state.installGuard(window);
  });
}

test('only owner replacements preserve a current route and history state', async ({ page }) => {
  await install(page);
  const result = await page.evaluate(() => {
    const { guard } = window as unknown as GuardWindow;
    return {
      saved: guard.replace('/en/modules/products?quantity=2#buyer'),
      href: location.href,
      forbiddenPath: guard.replace('/en/modules/cart#buyer'),
      forbiddenHash: guard.replace('/en/modules/products?quantity=2#other'),
      current: guard.current(),
    };
  });
  expect(result).toEqual({
    saved: true,
    href: 'https://route.test/en/modules/products?quantity=2#buyer',
    forbiddenPath: false,
    forbiddenHash: false,
    current: true,
  });
});

test('external same-path ABA invalidates before history returns and blocks stale writes', async ({ page }) => {
  await install(page);
  const result = await page.evaluate(() => {
    const state = window as unknown as GuardWindow;
    history.pushState(null, '', '?quantity=2#buyer');
    const invalidatedSynchronously = !state.guard.current() && state.invalidations === 1;
    history.replaceState(null, '', '?quantity=1#buyer');
    return {
      invalidatedSynchronously,
      current: state.guard.current(),
      saved: state.guard.replace('/en/modules/products?cart_id=stale#buyer'),
      href: location.href,
      invalidations: state.invalidations,
    };
  });
  expect(result).toEqual({
    invalidatedSynchronously: true,
    current: false,
    saved: false,
    href: 'https://route.test/en/modules/products?quantity=1#buyer',
    invalidations: 1,
  });
});
