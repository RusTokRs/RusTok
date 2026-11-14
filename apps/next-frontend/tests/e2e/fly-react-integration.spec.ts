/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

import { expect, test, type Page } from '@playwright/test';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { createRequire } from 'node:module';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// Bundle the real React owner and landing island with the app's existing webpack + TypeScript.
// Only router, translation lookup, and the network boundary are replaced by deterministic fakes.
const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
const fixtures = join(here, 'fixtures');
const require = createRequire(import.meta.url);
type Stats = { hasErrors(): boolean; toString(options: object): string };
const webpack = require('next/dist/compiled/webpack/webpack').webpack as (
  config: object, callback: (error: Error | null, stats?: Stats) => void,
) => void;
let output = '';
let bundle = '';

test.beforeAll(async () => {
  output = await mkdtemp(join(tmpdir(), 'fly-react-harness-'));
  const host = join(fixtures, 'fly-react-host.ts');
  await new Promise<void>((done, fail) => webpack({
    mode: 'development', devtool: false, context: root,
    entry: join(fixtures, 'fly-react-harness.tsx'),
    output: { path: output, filename: 'bundle.js' },
    module: { rules: [{
      test: /\.[jt]sx?$/, exclude: /node_modules/,
      use: [{ loader: join(fixtures, 'fly-react-loader.cjs') }],
    }] },
    resolve: {
      extensions: ['.tsx', '.ts', '.js'],
      modules: [join(root, 'node_modules'), 'node_modules'],
      alias: {
        'next/navigation$': host,
        'next/link$': join(fixtures, 'fly-react-link.tsx'),
        '@rustok/next-fluent$': host,
        '@/shared/lib/graphql$': host,
        '@rustok/cart-frontend$': join(root, 'packages/rustok-cart/src/index.tsx'),
        '@rustok/pricing-frontend$': join(root, 'packages/rustok-pricing/src/core.ts'),
        '@': join(root, 'src'),
      },
    },
  }, (error, stats) => {
    if (error) fail(error);
    else if (!stats || stats.hasErrors()) fail(new Error(stats?.toString({ all: false, errors: true, errorDetails: true }) ?? 'No webpack result'));
    else done();
  }));
  bundle = await readFile(join(output, 'bundle.js'), 'utf8');
});

test.afterAll(async () => { if (output) await rm(output, { recursive: true, force: true }); });

async function mount(page: Page) {
  await page.route('https://react.test/**', (route) => route.fulfill({
    contentType: 'text/html', body: '<!doctype html><html><body><div id="app"></div></body></html>',
  }));
  await page.goto('https://react.test/ru/pages/landing');
  await page.addScriptTag({ content: bundle });
  await expect(page.getByTestId('probe')).toHaveAttribute('data-enabled', 'false');
}

test('refreshed module props fence capability at commit, including equal-value refreshes', async ({ page }) => {
  await mount(page);
  await expect(page.getByTestId('module-probe')).toHaveAttribute('data-enabled', 'true');
  await page.getByTestId('server-capability').click();
  await expect(page.getByTestId('module-probe')).toHaveAttribute('data-enabled', 'false');
  expect(await page.evaluate(() => window.flyHarness.providerCommits)).toEqual(['true:0:true', 'false:0:false']);
  await page.getByTestId('module-override').click();
  await expect(page.getByTestId('module-probe')).toHaveAttribute('data-enabled', 'true');
  // Unrelated renders keep the same server prop and preserve the local override.
  await page.getByTestId('replace-landing').click();
  await expect(page.getByTestId('module-probe')).toHaveAttribute('data-enabled', 'true');
  // A fresh authoritative server prop resets it at commit, even with the same values.
  await page.getByTestId('refresh-server-modules').click();
  await expect(page.getByTestId('module-probe')).toHaveAttribute('data-enabled', 'false');
  await page.getByTestId('server-capability').click();
  await expect(page.getByTestId('module-probe')).toHaveAttribute('data-enabled', 'true');
  await page.getByTestId('server-capability').click();
  await expect(page.getByTestId('module-probe')).toHaveAttribute('data-enabled', 'false');
  expect(await page.evaluate(() => window.flyHarness.providerCommits)).toEqual([
    'true:0:true', 'false:0:false', 'false:0:true', 'false:1:false',
    'true:1:true', 'false:1:false',
  ]);
});

test('the React landing uses the real Cart capability and hands purchase to Product', async ({ page }) => {
  await mount(page);
  await expect(page.getByTestId('count')).toHaveText('—');
  await page.getByTestId('cart').click();
  await expect(page.getByRole('alert')).toHaveText('cartUnavailable');
  await page.getByTestId('invalid').click();
  await expect(page.getByRole('alert')).toHaveText('buyerHandoffError');

  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  await expect(page.getByTestId('count')).toHaveText('0');
  await page.getByTestId('cart').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-open', 'true');
  await expect(page.getByRole('alert')).toHaveCount(0);
  await page.keyboard.press('Escape');
  await page.getByTestId('buy').click();
  expect(await page.evaluate(() => window.flyHarness.navigation)).toEqual([
    '/ru/products/cup?currency=AED&variant_id=11111111-1111-4111-8111-111111111111&quantity=2',
  ]);
  expect(await page.evaluate(() => window.flyHarness.requests)).toEqual([]);
});

test('a capability disable updates the landing bridge before commit-phase interactions', async ({ page }) => {
  await mount(page);
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  await expect(page.getByTestId('count')).toHaveText('0');
  await page.evaluate(() => { window.flyHarness.clickCartOnDisable = true; });
  await page.getByTestId('capability').click();
  expect(await page.evaluate(() => window.flyHarness.counterAtDisable)).toBe('—');
  await expect(page.getByRole('alert')).toHaveText('cartUnavailable');
  await expect(page.getByTestId('probe')).toHaveAttribute('data-open', 'false');
});

test('a stale Cart mutation is fenced before child commit-phase effects run', async ({ page }) => {
  await mount(page);
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  await page.evaluate(() => {
    window.flyHarness.staleAdd = window.flyHarness.cart?.addItem ?? null;
    window.flyHarness.startStaleAddOnDisable = true;
  });
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-enabled', 'false');
  expect(await page.evaluate(() => window.flyHarness.requests)).toEqual([]);
  await expect.poll(() => page.evaluate(() => window.flyHarness.addResult)).toBe(false);
});

test('a captured Cart control is fenced after its capability scope expires', async ({ page }) => {
  await mount(page);
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  await page.evaluate(() => { window.flyHarness.staleToggle = window.flyHarness.cart?.toggleCart ?? null; });
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-enabled', 'false');
  await page.evaluate(async () => {
    window.flyHarness.staleToggle?.();
    await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
  });
  await expect(page.getByTestId('probe')).toHaveAttribute('data-open', 'false');
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  await page.evaluate(async () => {
    window.flyHarness.staleToggle?.();
    await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
  });
  await expect(page.getByTestId('probe')).toHaveAttribute('data-open', 'false');
  await page.getByTestId('cart').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-open', 'true');
});

test('unmounting Cart during create cannot continue the old add', async ({ page }) => {
  await mount(page);
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  await page.evaluate(() => {
    const cart = window.flyHarness.cart;
    if (!cart) throw new Error('Cart context not mounted');
    void cart.addItem('11111111-1111-4111-8111-111111111111', 2, {
      locale: 'ru', currencyCode: 'AED', regionId: null, priceListId: null,
      channelId: null, channelSlug: null, quantity: 2,
    }).then((value) => { window.flyHarness.addResult = value; });
  });
  await expect.poll(() => page.evaluate(() => !!window.flyHarness.resolveCreate)).toBe(true);
  await page.evaluate(() => {
    window.flyHarness.unmount?.();
    window.flyHarness.resolveCreate?.();
  });
  await expect.poll(() => page.evaluate(() => window.flyHarness.addResult)).toBe(false);
  expect(await page.evaluate(() => window.flyHarness.requests.some((query) => query.includes('AddStorefrontCartLineItem')))).toBe(false);
  expect(await page.evaluate(() => localStorage.getItem('rustok_storefront_cart_id:acme:guest')))
    .toBe('22222222-2222-4222-8222-222222222222');
  // The create response was confirmed and no add was sent, so this marker is settled.
  expect(await page.evaluate(() => localStorage.getItem('rustok_storefront_cart_id:acme:guest:mutation'))).toBeNull();
});

test('a remounted Cart waits for a known same-realm mutation before restoring', async ({ page }) => {
  await mount(page);
  await page.evaluate(() => {
    localStorage.setItem('rustok_storefront_cart_id:acme:guest', window.flyHarness.ownerCart.id);
    window.flyHarness.ownerCart.lineItems = [{
      id: '33333333-3333-4333-8333-333333333333', cartId: window.flyHarness.ownerCart.id,
      variantId: '11111111-1111-4111-8111-111111111111', title: 'Cup', quantity: 1,
      unitPrice: '10', totalPrice: '10', currencyCode: 'AED',
    }];
  });
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('count')).toHaveText('1');
  await page.evaluate(() => {
    const cart = window.flyHarness.cart;
    if (!cart) throw new Error('Cart context not mounted');
    void cart.updateQuantity('33333333-3333-4333-8333-333333333333', 2)
      .then((value) => { window.flyHarness.updateResult = value; });
  });
  await expect.poll(() => page.evaluate(() => !!window.flyHarness.resolveUpdate)).toBe(true);
  await page.evaluate(() => window.flyHarness.remount?.());
  await expect(page.getByTestId('probe')).toHaveAttribute('data-enabled', 'false');
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'true');
  await expect(page.getByTestId('count')).toHaveText('…');
  expect(await page.evaluate(() => localStorage.getItem('rustok_storefront_cart_id:acme:guest:mutation'))).not.toBeNull();
  expect(await page.evaluate(() => window.flyHarness.requests.filter((query) => query.includes('query StorefrontCart')).length)).toBe(1);

  await page.evaluate(() => window.flyHarness.resolveUpdate?.());
  await expect.poll(() => page.evaluate(() => window.flyHarness.updateResult)).toBe(false);
  await expect(page.getByTestId('count')).toHaveText('2');
  expect(await page.evaluate(() => window.flyHarness.cart?.actionError)).toBeNull();
  expect(await page.evaluate(() => localStorage.getItem('rustok_storefront_cart_id:acme:guest:mutation'))).toBeNull();
});

test('a persisted mutation marker is not automatically treated as a confirmed Cart', async ({ page }) => {
  await mount(page);
  await page.evaluate(() => {
    localStorage.setItem('rustok_storefront_cart_id:acme:guest', window.flyHarness.ownerCart.id);
    // Models an operation from a previous page or another tab, not a tracked promise here.
    localStorage.setItem('rustok_storefront_cart_id:acme:guest:mutation', 'unknown-operation');
  });
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  await expect(page.getByTestId('count')).toHaveText('—');
  expect(await page.evaluate(() => window.flyHarness.cart?.actionError)).toBe('unconfirmed');
  expect(await page.evaluate(() => window.flyHarness.requests)).toEqual([]);
  expect(await page.evaluate(() => localStorage.getItem('rustok_storefront_cart_id:acme:guest:mutation'))).toBe('unknown-operation');
});

test('inspection waits for a mutation known to another same-realm Cart provider', async ({ page }) => {
  await mount(page);
  await page.evaluate(() => {
    localStorage.setItem('rustok_storefront_cart_id:acme:guest', window.flyHarness.ownerCart.id);
    window.flyHarness.ownerCart.lineItems = [{
      id: '33333333-3333-4333-8333-333333333333', cartId: window.flyHarness.ownerCart.id,
      variantId: '11111111-1111-4111-8111-111111111111', title: 'Cup', quantity: 1,
      unitPrice: '10', totalPrice: '10', currencyCode: 'AED',
    }];
  });
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('count')).toHaveText('1');
  await page.getByTestId('show-second-cart').click();
  await expect(page.getByTestId('second-probe')).toHaveAttribute('data-count', '1');
  await page.evaluate(() => {
    const cart = window.flyHarness.cart;
    if (!cart) throw new Error('Cart context not mounted');
    void cart.updateQuantity('33333333-3333-4333-8333-333333333333', 2)
      .then((value) => { window.flyHarness.updateResult = value; });
  });
  await expect.poll(() => page.evaluate(() => !!window.flyHarness.resolveUpdate)).toBe(true);
  await page.evaluate(() => { void window.flyHarness.secondCart?.refreshCart(); });
  await expect(page.getByTestId('second-probe')).toHaveAttribute('data-updating', 'true');
  expect(await page.evaluate(() => window.flyHarness.requests.filter((query) => query.includes('query StorefrontCart')).length)).toBe(2);
  await page.evaluate(() => window.flyHarness.resolveUpdate?.());
  await expect.poll(() => page.evaluate(() => window.flyHarness.updateResult)).toBe(true);
  await expect(page.getByTestId('second-probe')).toHaveAttribute('data-count', '2');
  await expect(page.getByTestId('second-probe')).toHaveAttribute('data-updating', 'false');
});

test('a completed same-realm mutation invalidates an older concurrent restore', async ({ page }) => {
  await mount(page);
  await page.evaluate(() => {
    localStorage.setItem('rustok_storefront_cart_id:acme:guest', window.flyHarness.ownerCart.id);
    window.flyHarness.ownerCart.lineItems = [{
      id: '33333333-3333-4333-8333-333333333333', cartId: window.flyHarness.ownerCart.id,
      variantId: '11111111-1111-4111-8111-111111111111', title: 'Cup', quantity: 1,
      unitPrice: '10', totalPrice: '10', currencyCode: 'AED',
    }];
  });
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('count')).toHaveText('1');
  await page.evaluate(() => { window.flyHarness.deferRead = true; });
  await page.getByTestId('show-second-cart').click();
  await expect.poll(() => page.evaluate(() => !!window.flyHarness.resolveRead)).toBe(true);
  await page.evaluate(() => {
    const cart = window.flyHarness.cart;
    if (!cart) throw new Error('Cart context not mounted');
    void cart.updateQuantity('33333333-3333-4333-8333-333333333333', 2)
      .then((value) => { window.flyHarness.updateResult = value; });
  });
  await expect.poll(() => page.evaluate(() => !!window.flyHarness.resolveUpdate)).toBe(true);
  await page.evaluate(() => window.flyHarness.resolveUpdate?.());
  await expect.poll(() => page.evaluate(() => window.flyHarness.updateResult)).toBe(true);
  // The marker was already removed, so only the in-realm revision can reject this old read.
  expect(await page.evaluate(() => localStorage.getItem('rustok_storefront_cart_id:acme:guest:mutation'))).toBeNull();
  await page.evaluate(() => window.flyHarness.resolveRead?.());
  await expect(page.getByTestId('second-probe')).toHaveAttribute('data-loading', 'false');
  await expect(page.getByTestId('second-probe')).toHaveAttribute('data-error', 'unconfirmed');
  await expect(page.getByTestId('second-probe')).toHaveAttribute('data-count', '0');
});

test('inspection cannot publish a read superseded by a completed same-realm mutation', async ({ page }) => {
  await mount(page);
  await page.evaluate(() => {
    localStorage.setItem('rustok_storefront_cart_id:acme:guest', window.flyHarness.ownerCart.id);
    window.flyHarness.ownerCart.lineItems = [{
      id: '33333333-3333-4333-8333-333333333333', cartId: window.flyHarness.ownerCart.id,
      variantId: '11111111-1111-4111-8111-111111111111', title: 'Cup', quantity: 1,
      unitPrice: '10', totalPrice: '10', currencyCode: 'AED',
    }];
  });
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('count')).toHaveText('1');
  await page.getByTestId('show-second-cart').click();
  await expect(page.getByTestId('second-probe')).toHaveAttribute('data-count', '1');
  await page.evaluate(() => {
    window.flyHarness.deferRead = true;
    void window.flyHarness.secondCart?.refreshCart();
  });
  await expect.poll(() => page.evaluate(() => !!window.flyHarness.resolveRead)).toBe(true);
  await page.evaluate(() => {
    const cart = window.flyHarness.cart;
    if (!cart) throw new Error('Cart context not mounted');
    void cart.updateQuantity('33333333-3333-4333-8333-333333333333', 2)
      .then((value) => { window.flyHarness.updateResult = value; });
  });
  await expect.poll(() => page.evaluate(() => !!window.flyHarness.resolveUpdate)).toBe(true);
  await page.evaluate(() => window.flyHarness.resolveUpdate?.());
  await expect.poll(() => page.evaluate(() => window.flyHarness.updateResult)).toBe(true);
  await page.evaluate(() => window.flyHarness.resolveRead?.());
  await expect(page.getByTestId('second-probe')).toHaveAttribute('data-updating', 'false');
  await expect(page.getByTestId('second-probe')).toHaveAttribute('data-error', 'unconfirmed');
});

test('disabling Cart during create retains its owner reference but never continues the add', async ({ page }) => {
  await mount(page);
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  await page.evaluate(() => {
    const cart = window.flyHarness.cart;
    if (!cart) throw new Error('Cart context not mounted');
    void cart.addItem('11111111-1111-4111-8111-111111111111', 2, {
      locale: 'ru', currencyCode: 'AED', regionId: null, priceListId: null,
      channelId: null, channelSlug: null, quantity: 2,
    }).then((value) => { window.flyHarness.addResult = value; });
  });
  await expect.poll(() => page.evaluate(() => !!window.flyHarness.resolveCreate)).toBe(true);
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-enabled', 'false');
  await page.evaluate(() => window.flyHarness.resolveCreate?.());
  await expect.poll(() => page.evaluate(() => window.flyHarness.addResult)).toBe(false);
  expect(await page.evaluate(() => window.flyHarness.requests.some((query) => query.includes('AddStorefrontCartLineItem')))).toBe(false);
  expect(await page.evaluate(() => localStorage.getItem('rustok_storefront_cart_id:acme:guest')))
    .toBe('22222222-2222-4222-8222-222222222222');
  expect(await page.evaluate(() => localStorage.getItem('rustok_storefront_cart_id:acme:guest:mutation'))).toBeNull();

  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  await expect(page.getByTestId('probe')).toHaveAttribute('data-reference', '22222222-2222-4222-8222-222222222222');
  expect(await page.evaluate(() => localStorage.getItem('rustok_storefront_cart_id:acme:guest:mutation'))).toBeNull();
  expect(await page.evaluate(() => window.flyHarness.requests.some((query) => query.includes('AddStorefrontCartLineItem')))).toBe(false);
});

test('Cart capability A→B→A fences the old create and restores after it settles', async ({ page }) => {
  await mount(page);
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  await page.evaluate(() => {
    const cart = window.flyHarness.cart;
    if (!cart) throw new Error('Cart context not mounted');
    void cart.addItem('11111111-1111-4111-8111-111111111111', 1, {
      locale: 'ru', currencyCode: 'AED', regionId: null, priceListId: null,
      channelId: null, channelSlug: null, quantity: 1,
    }).then((value) => { window.flyHarness.addResult = value; });
  });
  await expect.poll(() => page.evaluate(() => !!window.flyHarness.resolveCreate)).toBe(true);
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-enabled', 'false');
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-enabled', 'true');
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'true');
  await page.evaluate(() => window.flyHarness.resolveCreate?.());
  await expect.poll(() => page.evaluate(() => window.flyHarness.addResult)).toBe(false);
  expect(await page.evaluate(() => window.flyHarness.requests.some((query) => query.includes('AddStorefrontCartLineItem')))).toBe(false);
  expect(await page.evaluate(() => localStorage.getItem('rustok_storefront_cart_id:acme:guest')))
    .toBe('22222222-2222-4222-8222-222222222222');

  // The new capability generation restores through Cart after the previous create settles.
  await expect(page.getByTestId('probe')).toHaveAttribute('data-reference', '22222222-2222-4222-8222-222222222222');
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  expect(await page.evaluate(() => localStorage.getItem('rustok_storefront_cart_id:acme:guest:mutation'))).toBeNull();
});

test('a previous-scope mutation cannot make a stale owner read look confirmed', async ({ page }) => {
  await mount(page);
  await page.evaluate(() => {
    localStorage.setItem('rustok_storefront_cart_id:acme:guest', window.flyHarness.ownerCart.id);
    window.flyHarness.ownerCart.lineItems = [{
      id: '33333333-3333-4333-8333-333333333333', cartId: window.flyHarness.ownerCart.id,
      variantId: '11111111-1111-4111-8111-111111111111', title: 'Cup', quantity: 1,
      unitPrice: '10', totalPrice: '10', currencyCode: 'AED',
    }];
  });
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('count')).toHaveText('1');
  await page.evaluate(() => {
    const cart = window.flyHarness.cart;
    if (!cart) throw new Error('Cart context not mounted');
    void cart.updateQuantity('33333333-3333-4333-8333-333333333333', 2)
      .then((value) => { window.flyHarness.updateResult = value; });
  });
  await expect.poll(() => page.evaluate(() => !!window.flyHarness.resolveUpdate)).toBe(true);
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-enabled', 'false');
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-enabled', 'true');

  // No restore may confirm a pre-mutation read while its previous-scope write is pending.
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'true');
  expect(await page.evaluate(() => localStorage.getItem('rustok_storefront_cart_id:acme:guest:mutation'))).not.toBeNull();
  await page.evaluate(() => window.flyHarness.resolveUpdate?.());
  await expect.poll(() => page.evaluate(() => window.flyHarness.updateResult)).toBe(false);
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  await expect(page.getByTestId('count')).toHaveText('2');
  expect(await page.evaluate(() => localStorage.getItem('rustok_storefront_cart_id:acme:guest:mutation'))).toBeNull();
});

test('a lost mutation response remains unconfirmed across a capability cycle', async ({ page }) => {
  await mount(page);
  await page.evaluate(() => {
    localStorage.setItem('rustok_storefront_cart_id:acme:guest', window.flyHarness.ownerCart.id);
    window.flyHarness.failUpdateResponse = true;
    window.flyHarness.ownerCart.lineItems = [{
      id: '33333333-3333-4333-8333-333333333333', cartId: window.flyHarness.ownerCart.id,
      variantId: '11111111-1111-4111-8111-111111111111', title: 'Cup', quantity: 1,
      unitPrice: '10', totalPrice: '10', currencyCode: 'AED',
    }];
  });
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('count')).toHaveText('1');
  await page.evaluate(() => {
    const cart = window.flyHarness.cart;
    if (!cart) throw new Error('Cart context not mounted');
    void cart.updateQuantity('33333333-3333-4333-8333-333333333333', 2)
      .then((value) => { window.flyHarness.updateResult = value; });
  });
  await expect.poll(() => page.evaluate(() => !!window.flyHarness.resolveUpdate)).toBe(true);
  await page.getByTestId('capability').click();
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'true');
  await page.evaluate(() => window.flyHarness.resolveUpdate?.());
  await expect.poll(() => page.evaluate(() => window.flyHarness.updateResult)).toBe(false);
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  await expect(page.getByTestId('count')).toHaveText('—');
  expect(await page.evaluate(() => window.flyHarness.cart?.actionError)).toBe('unconfirmed');
  expect(await page.evaluate(() => window.flyHarness.requests.filter((query) => query.includes('query StorefrontCart')).length)).toBe(1);
  expect(await page.evaluate(() => localStorage.getItem('rustok_storefront_cart_id:acme:guest:mutation'))).not.toBeNull();
});

test('a replaced landing observes the current Cart state without waiting for another update', async ({ page }) => {
  await mount(page);
  await page.evaluate(() => {
    localStorage.setItem('rustok_storefront_cart_id:acme:guest', window.flyHarness.ownerCart.id);
    window.flyHarness.ownerCart.lineItems = [{
      id: '33333333-3333-4333-8333-333333333333', cartId: window.flyHarness.ownerCart.id,
      variantId: '11111111-1111-4111-8111-111111111111', title: 'Cup', quantity: 3,
      unitPrice: '10', totalPrice: '30', currencyCode: 'AED',
    }];
  });
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('count')).toHaveText('3');
  await page.getByTestId('replace-landing').click();
  await expect(page.getByTestId('replacement-count')).toHaveText('3');
});

test('same-actor auth interruption hides the old Cart until a fresh owner read completes', async ({ page }) => {
  await mount(page);
  await page.evaluate(() => {
    localStorage.setItem('rustok_storefront_cart_id:acme:guest', window.flyHarness.ownerCart.id);
    window.flyHarness.ownerCart.lineItems = [{
      id: '33333333-3333-4333-8333-333333333333', cartId: window.flyHarness.ownerCart.id,
      variantId: '11111111-1111-4111-8111-111111111111', title: 'Cup', quantity: 1,
      unitPrice: '10', totalPrice: '10', currencyCode: 'AED',
    }];
  });
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('count')).toHaveText('1');
  await page.getByTestId('show-checkout').click();
  const checkout = page.getByTestId('checkout');
  await expect(checkout.locator('form')).toHaveCount(1);

  await page.getByTestId('auth').click();
  await expect(page.getByTestId('count')).toHaveText('—');
  await expect(checkout.getByRole('alert')).toHaveText('authError');
  await page.evaluate(() => {
    window.flyHarness.deferRead = true;
    window.flyHarness.ownerCart.lineItems[0].quantity = 2;
  });
  await page.getByTestId('auth').click();
  await expect.poll(() => page.evaluate(() => !!window.flyHarness.resolveRead)).toBe(true);
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'true');
  await expect(page.getByTestId('count')).toHaveText('…');
  await expect(checkout.locator('form')).toHaveCount(0);
  await page.evaluate(() => window.flyHarness.resolveRead?.());
  await expect(page.getByTestId('count')).toHaveText('2');
  await expect(checkout.locator('form')).toHaveCount(1);
});

test('auth A→B→A during create cannot continue the previous add', async ({ page }) => {
  await mount(page);
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  await page.evaluate(() => {
    const cart = window.flyHarness.cart;
    if (!cart) throw new Error('Cart context not mounted');
    void cart.addItem('11111111-1111-4111-8111-111111111111', 2, {
      locale: 'ru', currencyCode: 'AED', regionId: null, priceListId: null,
      channelId: null, channelSlug: null, quantity: 2,
    }).then((value) => { window.flyHarness.addResult = value; });
  });
  await expect.poll(() => page.evaluate(() => !!window.flyHarness.resolveCreate)).toBe(true);
  await page.getByTestId('auth').click();
  await expect(page.getByTestId('count')).toHaveText('—');
  await page.getByTestId('auth').click();
  await page.evaluate(() => window.flyHarness.resolveCreate?.());
  await expect.poll(() => page.evaluate(() => window.flyHarness.addResult)).toBe(false);
  expect(await page.evaluate(() => window.flyHarness.addVariables)).toBeNull();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  expect(await page.evaluate(() => localStorage.getItem('rustok_storefront_cart_id:acme:guest')))
    .toBe('22222222-2222-4222-8222-222222222222');
});

test('the landing cart reports a failed owner read instead of a false empty cart', async ({ page }) => {
  await mount(page);
  await page.evaluate(() => {
    localStorage.setItem('rustok_storefront_cart_id:acme:guest', window.flyHarness.ownerCart.id);
    window.flyHarness.failRead = true;
  });
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('count')).toHaveText('—');
  await page.getByTestId('cart').click();
  const drawer = page.getByRole('dialog');
  await expect(drawer.getByRole('alert')).toHaveText('restoreError');
  await expect(drawer.getByText('Ваша корзина пуста')).toHaveCount(0);
  await page.evaluate(() => { window.flyHarness.failRead = false; });
  await drawer.getByRole('button', { name: 'inspect' }).click();
  await expect(drawer.getByText('Ваша корзина пуста')).toBeVisible();
  await expect(page.getByTestId('count')).toHaveText('0');
});

test('an unconfirmed Cart mutation hides stale checkout and can be inspected without retrying', async ({ page }) => {
  await mount(page);
  await page.evaluate(() => {
    localStorage.setItem('rustok_storefront_cart_id:acme:guest', window.flyHarness.ownerCart.id);
    window.flyHarness.failUpdateResponse = true;
    window.flyHarness.ownerCart.lineItems = [{
      id: '33333333-3333-4333-8333-333333333333', cartId: window.flyHarness.ownerCart.id,
      variantId: '11111111-1111-4111-8111-111111111111', title: 'Cup', quantity: 1,
      unitPrice: '10', totalPrice: '10', currencyCode: 'AED',
    }];
  });
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('count')).toHaveText('1');
  await page.evaluate(() => {
    const cart = window.flyHarness.cart;
    if (!cart) throw new Error('Cart context not mounted');
    void cart.updateQuantity('33333333-3333-4333-8333-333333333333', 2)
      .then((value) => { window.flyHarness.updateResult = value; });
  });
  await expect.poll(() => page.evaluate(() => !!window.flyHarness.resolveUpdate)).toBe(true);
  await page.evaluate(() => window.flyHarness.resolveUpdate?.());
  await expect.poll(() => page.evaluate(() => window.flyHarness.updateResult)).toBe(false);
  await expect(page.getByTestId('count')).toHaveText('—');
  await expect(page.getByRole('button', { name: 'Открыть корзину' }).getByRole('status')).toHaveText('—');
  await page.getByTestId('show-checkout').click();
  const checkout = page.getByTestId('checkout');
  await expect(checkout.getByRole('alert')).toHaveText('unconfirmedError');
  await expect(checkout.locator('form')).toHaveCount(0);
  await page.getByTestId('cart').click();
  const drawer = page.getByRole('dialog');
  await expect(drawer.getByRole('alert')).toHaveText('unconfirmedError');
  await expect(drawer.getByRole('link', { name: /Оформить заказ/ })).toHaveCount(0);
  await drawer.getByRole('button', { name: 'inspect' }).click();
  await expect(drawer.getByRole('alert')).toHaveCount(0);
  await expect(page.getByTestId('count')).toHaveText('2');
  await expect(checkout.locator('form')).toHaveCount(1);
  expect(await page.evaluate(() => window.flyHarness.requests.filter((query) => query.includes('mutation UpdateStorefrontCartLineItem')).length)).toBe(1);
});

test('an unknown Cart creation requires an explicit local reset when no reference exists', async ({ page }) => {
  await mount(page);
  await page.evaluate(() => {
    localStorage.setItem('rustok_storefront_cart_id:acme:guest:mutation', 'pending-create');
  });
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('count')).toHaveText('—');
  await page.getByTestId('cart').click();
  const drawer = page.getByRole('dialog');
  await expect(drawer.getByRole('alert')).toHaveText('unconfirmedError');
  await expect(drawer.getByRole('button', { name: 'inspect' })).toHaveCount(0);
  await expect(drawer.getByText('forgetWarning')).toBeVisible();
  await drawer.getByRole('button', { name: 'forgetReference' }).click();
  await expect(drawer.getByText('Ваша корзина пуста')).toBeVisible();
  expect(await page.evaluate(() => localStorage.getItem('rustok_storefront_cart_id:acme:guest:mutation'))).toBeNull();
});

test('the Product buyer honors Cart capability after a landing handoff and requires confirmation', async ({ page }) => {
  await mount(page);
  await page.getByTestId('buy').click();
  expect(await page.evaluate(() => window.flyHarness.requests)).toEqual([]);
  await page.getByTestId('show-buyer').click();
  const buyer = page.getByTestId('buyer');
  await expect(buyer.getByText('buyerReady')).toBeVisible();
  await expect(buyer.getByRole('button', { name: 'addToCart' })).toBeDisabled();
  await expect(buyer.getByText('disabledError')).toBeVisible();
  expect(await page.evaluate(() => window.flyHarness.addVariables)).toBeNull();

  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  await expect(buyer.getByText('buyerReady')).toBeVisible();
  const add = buyer.getByRole('button', { name: 'addToCart' });
  await expect(add).toBeEnabled();
  expect(await page.evaluate(() => window.flyHarness.addVariables)).toBeNull();
  await add.click();
  await expect.poll(() => page.evaluate(() => !!window.flyHarness.resolveCreate)).toBe(true);
  expect(await page.evaluate(() => window.flyHarness.addVariables)).toBeNull();
  await page.evaluate(() => window.flyHarness.resolveCreate?.());
  await expect.poll(() => page.evaluate(() => window.flyHarness.addVariables)).not.toBeNull();
  expect(await page.evaluate(() => window.flyHarness.addVariables)).toMatchObject({
    cartId: '22222222-2222-4222-8222-222222222222',
    input: {
      variantId: '11111111-1111-4111-8111-111111111111', quantity: 2,
      expectedLocale: 'ru', expectedContext: { currencyCode: 'AED', quantity: 2 },
    },
  });
  await expect(page.getByTestId('count')).toHaveText('2');
});

test('Product does not continue an add after Cart is disabled during creation', async ({ page }) => {
  await mount(page);
  await page.getByTestId('buy').click();
  await page.getByTestId('show-buyer').click();
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  const buyer = page.getByTestId('buyer');
  const add = buyer.getByRole('button', { name: 'addToCart' });
  await expect(add).toBeEnabled();
  await add.click();
  await expect.poll(() => page.evaluate(() => !!window.flyHarness.resolveCreate)).toBe(true);

  await page.getByTestId('capability').click();
  await expect(buyer.getByText('disabledError')).toBeVisible();
  await expect(add).toBeDisabled();
  await page.evaluate(() => window.flyHarness.resolveCreate?.());
  await expect.poll(() => page.evaluate(() => localStorage.getItem('rustok_storefront_cart_id:acme:guest')))
    .toBe('22222222-2222-4222-8222-222222222222');
  expect(await page.evaluate(() => window.flyHarness.addVariables)).toBeNull();
  await expect(page.getByTestId('count')).toHaveText('—');

  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  await expect(add).toBeEnabled();
  expect(await page.evaluate(() => window.flyHarness.addVariables)).toBeNull();
  // Restore does not retry a purchase: a new explicit confirmation is required.
  await add.click();
  await expect.poll(() => page.evaluate(() => window.flyHarness.addVariables)).not.toBeNull();
  await expect(page.getByTestId('count')).toHaveText('2');
});

test('Product ignores old quotes across disabled → enabled → disabled → enabled', async ({ page }) => {
  await mount(page);
  await page.getByTestId('show-buyer').click();
  const buyer = page.getByTestId('buyer');
  await expect(buyer.getByText('10.00 AED')).toBeVisible();
  await page.evaluate(() => { window.flyHarness.deferQuote = true; });

  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  await expect.poll(() => page.evaluate(() => window.flyHarness.quoteResolvers.length)).toBeGreaterThan(0);
  const enabledReads = await page.evaluate(() => window.flyHarness.quoteResolvers.length);
  await expect(buyer.getByText('buyerLoading')).toBeVisible();
  await expect(buyer.getByText('10.00 AED')).toHaveCount(0);
  await expect(buyer.getByRole('button', { name: 'addToCart' })).toBeDisabled();

  await page.getByTestId('capability').click();
  await expect.poll(() => page.evaluate(() => window.flyHarness.quoteResolvers.length)).toBeGreaterThan(enabledReads);
  await expect(buyer.getByText('disabledError')).toBeVisible();
  const disabledReads = await page.evaluate(() => window.flyHarness.quoteResolvers.length);
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  await expect.poll(() => page.evaluate(() => window.flyHarness.quoteResolvers.length)).toBeGreaterThan(disabledReads);

  // Fulfil all earlier scopes after the latest request has started. None can publish money or
  // unlock the buyer, even when the capability returns to its earlier boolean value.
  await page.evaluate(() => {
    const old = structuredClone(window.flyHarness.productQuote);
    if (!old.variants[0].effectivePrice) throw new Error('Missing test price');
    old.variants[0].effectivePrice.amount = '999.00';
    window.flyHarness.quoteResolvers.slice(0, -1).forEach((resolve) => resolve(old));
  });
  await expect(buyer.getByText('buyerLoading')).toBeVisible();
  await expect(buyer.getByText('999.00 AED')).toHaveCount(0);
  await expect(buyer.getByRole('button', { name: 'addToCart' })).toBeDisabled();

  await page.evaluate(() => {
    const current = structuredClone(window.flyHarness.productQuote);
    if (!current.variants[0].effectivePrice) throw new Error('Missing test price');
    current.variants[0].effectivePrice.amount = '12.00';
    window.flyHarness.quoteResolvers.at(-1)?.(current);
  });
  await expect(buyer.getByText('12.00 AED')).toBeVisible();
  await expect(buyer.getByRole('button', { name: 'addToCart' })).toBeEnabled();
  expect(await page.evaluate(() => window.flyHarness.addVariables)).toBeNull();
});

test('the drawer does not claim an empty cart while its first create is pending', async ({ page }) => {
  await mount(page);
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  await page.getByTestId('cart').click();
  const drawer = page.getByRole('dialog');
  await expect(drawer.getByText('Ваша корзина пуста')).toBeVisible();
  await page.evaluate(() => {
    const cart = window.flyHarness.cart;
    if (!cart) throw new Error('Cart context not mounted');
    void cart.addItem('11111111-1111-4111-8111-111111111111', 2, {
      locale: 'ru', currencyCode: 'AED', regionId: null, priceListId: null,
      channelId: null, channelSlug: null, quantity: 2,
    });
  });
  await expect.poll(() => page.evaluate(() => !!window.flyHarness.resolveCreate)).toBe(true);
  await expect(drawer.getByText('Ваша корзина пуста')).toHaveCount(0);
  await expect(drawer.getByText('loading').first()).toBeVisible();
  await expect(drawer.getByRole('link', { name: /Оформить заказ/ })).toHaveCount(0);
});

test('the drawer hides stale items and checkout while a Cart update is pending', async ({ page }) => {
  await mount(page);
  await page.evaluate(() => {
    localStorage.setItem('rustok_storefront_cart_id:acme:guest', window.flyHarness.ownerCart.id);
    window.flyHarness.ownerCart.lineItems = [{
      id: '33333333-3333-4333-8333-333333333333', cartId: window.flyHarness.ownerCart.id,
      variantId: '11111111-1111-4111-8111-111111111111', title: 'Cup', quantity: 1,
      unitPrice: '10', totalPrice: '10', currencyCode: 'AED',
    }];
  });
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('count')).toHaveText('1');
  await page.getByTestId('cart').click();
  const drawer = page.getByRole('dialog');
  await expect(drawer.getByText('Cup')).toBeVisible();
  await page.evaluate(() => {
    const cart = window.flyHarness.cart;
    if (!cart) throw new Error('Cart context not mounted');
    void cart.updateQuantity('33333333-3333-4333-8333-333333333333', 2);
  });
  await expect.poll(() => page.evaluate(() => !!window.flyHarness.resolveUpdate)).toBe(true);
  await expect(drawer.getByText('Cup')).toHaveCount(0);
  await expect(drawer.getByText('loading').first()).toBeVisible();
  await expect(drawer.getByRole('link', { name: /Оформить заказ/ })).toHaveCount(0);
  await page.evaluate(() => window.flyHarness.resolveUpdate?.());
  await expect(drawer.getByText('Cup')).toBeVisible();
  await expect(drawer.getByRole('link', { name: /Оформить заказ/ })).toBeVisible();
});

test('a mounted drawer reports disabled capability rather than a false empty cart', async ({ page }) => {
  await mount(page);
  await page.getByTestId('capability').click();
  await expect(page.getByTestId('probe')).toHaveAttribute('data-loading', 'false');
  await page.getByTestId('cart').click();
  const drawer = page.getByRole('dialog');
  await page.getByTestId('capability').evaluate((button) => (button as HTMLElement).click());
  await expect(drawer.getByRole('alert')).toHaveText('disabledError');
  await expect(drawer.getByText('Ваша корзина пуста')).toHaveCount(0);
});

test('checkout is unavailable rather than empty when the host disables Cart', async ({ page }) => {
  await mount(page);
  await page.getByTestId('show-checkout').click();
  const checkout = page.getByTestId('checkout');
  await expect(checkout.getByRole('alert')).toHaveText('disabledError');
  await expect(checkout.locator('form')).toHaveCount(0);
  await expect(checkout.getByText('Ваша корзина пуста')).toHaveCount(0);
});
