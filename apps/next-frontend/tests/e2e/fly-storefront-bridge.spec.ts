import { expect, test, type Page } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { stripTypeScriptTypes } from 'node:module';

const rustBridge = new URL('../../../../crates/modules/rustok-page-builder/storefront/assets/fly-storefront-bridge.js', import.meta.url);
const nextBridge = new URL('../../packages/rustok-pages/src/components/landing-bridge.ts', import.meta.url);
const runtime = new URL('../../../../crates/ui/fly/browser/assets/fly-actions.js', import.meta.url);

type BridgeTestWindow = Window & {
  installLandingBridge(root: HTMLElement, handlers: Record<string, (...args: unknown[]) => void>): () => void;
  safeNavigationHref(raw: unknown): string | null;
  bridgeEvents: unknown[][];
  bridgeDisposers: Array<() => void>;
  flyXss?: boolean;
  scrolls: string[];
};

async function mount(page: Page): Promise<void> {
  await page.route('https://landing.test/**', (route) => route.fulfill({
    contentType: 'text/html',
    body: '<!doctype html><html><body></body></html>',
  }));
  await page.goto('https://landing.test/page');
  await page.setContent(`<!doctype html><html><body>
    <a href="#destination" data-testid="host-link">Host link</a>
    <div id="destination" data-testid="host-destination">Host destination</div>
    <span data-fly-island="cart_count" data-testid="host-count">host</span>
    <section data-testid="one-surface">
      <main data-fly-page-body class="fly-page-body" data-testid="one">
        <a href="#destination" data-testid="one-link">Landing link</a>
        <a href="/shop" data-testid="one-shop">Shop</a>
        <a href="/shop" target="_parent" data-testid="other-target">Other target</a>
        <div id="destination" data-testid="one-destination">Landing destination</div>
        <span data-fly-island="cart_count" data-testid="one-count">0</span>
        <button data-fly-island="cart_trigger" data-testid="one-cart">Cart</button>
        <button data-testid="one-action">Action</button>
      </main>
      <p data-fly-bridge-error="buyer_handoff_invalid" role="alert" hidden>Invalid purchase link</p>
      <p data-fly-bridge-error="cart_unavailable" role="alert" hidden>Cart unavailable</p>
    </section>
    <section data-testid="two-surface">
      <main data-fly-page-body class="fly-page-body" data-testid="two">
        <span data-fly-island="cart_count" data-testid="two-count">0</span>
        <button data-testid="two-action">Action</button>
      </main>
      <p data-fly-bridge-error="buyer_handoff_invalid" role="alert" hidden>Invalid purchase link</p>
      <p data-fly-bridge-error="cart_unavailable" role="alert" hidden>Cart unavailable</p>
    </section>
  </body></html>`);
  await page.evaluate(() => {
    const state = window as unknown as BridgeTestWindow;
    state.scrolls = [];
    for (const el of document.querySelectorAll('[id="destination"]')) {
      (el as HTMLElement).scrollIntoView = () => state.scrolls.push(el.getAttribute('data-testid')!);
    }
  });
  await page.addScriptTag({ content: await readFile(runtime, 'utf8') });
}

async function installNext(page: Page): Promise<void> {
  // Execute the actual bridge module in the browser with mock host capabilities.
  const source = stripTypeScriptTypes(await readFile(nextBridge, 'utf8'));
  await page.addScriptTag({ type: 'module', content: `${source}\nwindow.installLandingBridge = installLandingBridge;\nwindow.safeNavigationHref = safeNavigationHref;` });
  await page.waitForFunction(() => typeof (window as unknown as BridgeTestWindow).installLandingBridge === 'function');
  await page.evaluate(() => {
    const state = window as unknown as BridgeTestWindow;
    state.bridgeEvents = [];
    const handlers = (name: string) => ({
      openCart: () => state.bridgeEvents.push([name, 'openCart']),
      closeCart: () => state.bridgeEvents.push([name, 'closeCart']),
      toggleCart: () => state.bridgeEvents.push([name, 'toggleCart']),
      navigate: (href: unknown) => state.bridgeEvents.push([name, 'navigate', href]),
      reportError: (code: unknown) => state.bridgeEvents.push([name, 'error', code]),
      clearError: () => {},
    });
    state.bridgeDisposers = [
      state.installLandingBridge(document.querySelector('[data-testid="one"]')!, handlers('one')),
      state.installLandingBridge(document.querySelector('[data-testid="two"]')!, handlers('two')),
    ];
  });
}

async function setAction(page: Page, testId: string, payload: Record<string, unknown>): Promise<void> {
  await page.getByTestId(testId).evaluate((el, value) => el.setAttribute('data-fly-action', JSON.stringify(value)), payload);
}

test('Next bridge isolates instances, host links and cart actions; cleanup removes listeners', async ({ page }) => {
  await mount(page);
  await installNext(page);
  await setAction(page, 'one-action', {
    kind: 'emit_event', event: 'add_to_cart',
    payload: { buyerHref: '/en/products/cup?currency=AED', variantId: '11111111-1111-4111-8111-111111111111', quantity: 2 },
  });
  await page.getByTestId('one-action').click();
  await page.getByTestId('one-cart').click();
  await page.getByTestId('one-link').click();
  await expect.poll(() => page.evaluate(() => (window as unknown as BridgeTestWindow).bridgeEvents)).toEqual([
    ['one', 'navigate', '/en/products/cup?currency=AED&variant_id=11111111-1111-4111-8111-111111111111&quantity=2'], ['one', 'toggleCart'],
  ]);
  await expect.poll(() => page.evaluate(() => (window as unknown as BridgeTestWindow).scrolls)).toEqual(['one-destination']);

  // A host link is not handled by a landing bridge. Dispatch rather than actually navigating.
  const probeClick = async (testId: string, options: { ctrlKey?: boolean } = {}) => page.getByTestId(testId).evaluate((el, init) => {
    let preventedByBridge = false;
    document.addEventListener('click', (event) => {
      preventedByBridge = event.defaultPrevented;
      event.preventDefault(); // Do not let this test actually navigate.
    }, { once: true });
    el.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, ...init }));
    return preventedByBridge;
  }, options);
  expect(await probeClick('host-link')).toBe(false);
  expect(await probeClick('one-shop', { ctrlKey: true })).toBe(false);
  expect(await probeClick('other-target')).toBe(false);

  await page.evaluate(() => (window as unknown as BridgeTestWindow).bridgeDisposers[0]());
  await page.getByTestId('one-cart').click();
  expect(await page.evaluate(() => (window as unknown as BridgeTestWindow).bridgeEvents)).toHaveLength(2);
});

test('Next bridge rejects dangerous custom navigation and allows supported URLs', async ({ page }) => {
  await mount(page);
  await installNext(page);
  for (const href of ['javascript:alert(1)', 'JaVaScRiPt:alert(1)', 'data:text/html,hi', '//evil.test', '/\\evil.test', 'https://bad host', 'vbscript:hello']) {
    await setAction(page, 'one-action', { kind: 'emit_event', event: 'navigate', payload: { href } });
    await page.getByTestId('one-action').click();
  }
  expect(await page.evaluate(() => (window as unknown as BridgeTestWindow).bridgeEvents)).toEqual([]);
  for (const href of ['/shop?tag=1', '#destination', '?q=one', 'https://example.test/path', 'mailto:hi@example.test', 'tel:+123']) {
    await setAction(page, 'one-action', { kind: 'emit_event', event: 'navigate', payload: { href } });
    await page.getByTestId('one-action').click();
  }
  expect(await page.evaluate(() => (window as unknown as BridgeTestWindow).bridgeEvents)).toEqual([
    ['one', 'navigate', '/shop?tag=1'], ['one', 'navigate', '#destination'],
    ['one', 'navigate', '?q=one'], ['one', 'navigate', 'https://example.test/path'],
    ['one', 'navigate', 'mailto:hi@example.test'], ['one', 'navigate', 'tel:+123'],
  ]);
});

test('Leptos bridge blocks javascript navigation, isolates anchors and installs once', async ({ page }) => {
  await mount(page);
  const source = await readFile(rustBridge, 'utf8');
  await page.addScriptTag({ content: source });
  await page.addScriptTag({ content: source });
  await page.evaluate(() => {
    const state = window as unknown as BridgeTestWindow;
    state.bridgeEvents = [];
    window.addEventListener('rustok:cart:add', (event) => state.bridgeEvents.push(['add', (event as CustomEvent).detail]));
    window.addEventListener('rustok:cart:toggle', (event) => {
      state.bridgeEvents.push(['toggle']);
      event.preventDefault(); // The Cart owner acknowledges that it handled the request.
    });
  });
  await setAction(page, 'one-action', { kind: 'emit_event', event: 'navigate', payload: { href: 'javascript:window.flyXss=true' } });
  await page.getByTestId('one-action').click();
  await page.waitForTimeout(100);
  expect(await page.evaluate(() => (window as unknown as BridgeTestWindow).flyXss)).toBeUndefined();
  expect(page.url()).toBe('https://landing.test/page');

  await expect(page.getByTestId('one-count')).toHaveText('—');
  await setAction(page, 'one-action', { kind: 'emit_event', event: 'cart:add', payload: { variantId: 'sku-1', quantity: 2 } });
  await page.getByTestId('one-action').click();
  await expect(page.getByTestId('one-surface').locator('[data-fly-bridge-error="buyer_handoff_invalid"]')).toBeVisible();
  await page.getByTestId('one-cart').click();
  await expect(page.getByTestId('one-surface').locator('[data-fly-bridge-error="buyer_handoff_invalid"]')).toBeHidden();
  await page.getByTestId('one-link').click();
  expect(await page.evaluate(() => (window as unknown as BridgeTestWindow).bridgeEvents)).toEqual([['toggle']]);
  expect(await page.evaluate(() => (window as unknown as BridgeTestWindow).scrolls)).toEqual(['one-destination']);
  const hostPrevented = await page.getByTestId('host-link').evaluate((el) => !el.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true })));
  expect(hostPrevented).toBe(false);
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('rustok:cart:updated', { detail: { count: 5 } })));
  await expect(page.getByTestId('one-count')).toHaveText('5');
  await expect(page.getByTestId('two-count')).toHaveText('5');
  await expect(page.getByTestId('host-count')).toHaveText('host');
});

const variantId = '11111111-1111-4111-8111-111111111111';
const invalidPurchases: Record<string, unknown>[] = [
  { productId: variantId, buyerHref: '/en/products/cup' },
  { id: variantId, buyerHref: '/en/products/cup' },
  { variantId, quantity: 1 },
  { variantId: '00000000-0000-0000-0000-000000000000', buyerHref: '/en/products/cup' },
  { variantId: 'not-a-sku', buyerHref: '/en/products/cup' },
  { variantId, buyerHref: 'javascript:alert(1)' },
  { variantId, buyerHref: '//evil.test/products/cup' },
  { variantId, buyerHref: '/\\evil.test/products/cup' },
  { variantId, buyerHref: '/en/products/cup?handle=other' },
  { variantId, buyerHref: '/en/products/cup?variant_id=other' },
  { variantId, buyerHref: '/en/products/cup?quantity=4' },
  { variantId, buyerHref: '/en/products/cup?cart_id=other' },
  { variantId, buyerHref: '/en/products/cup?axis.x=y' },
  { variantId, buyerHref: '/en/products/cup?currency=EUR&currency=USD' },
  ...[null, false, 0, -1, 1.5, 2147483648, '', '1e2', ' 2', '2.5'].map((quantity) => ({
    variantId, buyerHref: '/en/products/cup', quantity,
  })),
];

for (const host of ['Next', 'Leptos'] as const) {
  test(`${host} rejects ambiguous purchases instead of mutating a cart`, async ({ page }) => {
    await mount(page);
    if (host === 'Next') await installNext(page);
    else {
      await page.addScriptTag({ content: await readFile(rustBridge, 'utf8') });
      await page.evaluate(() => {
        const state = window as unknown as BridgeTestWindow;
        state.bridgeEvents = [];
        window.addEventListener('rustok:cart:add', (event) => state.bridgeEvents.push(['add', (event as CustomEvent).detail]));
      });
    }
    // Both hosts validate the same canonical Product URL before Leptos translates it to its
    // module route. These cases therefore exercise payload checks on both implementations.
    const wrongRoutes = ['/ru/pages/not-product', '/ru/modules/products?handle=cup',
      '/ru/products', '/ru/PRODUCTS/cup', '/ru/products/cup/extra',
      '/ru/products/%ZZ', '/ru/products/cup%2Fother', '/en%2Ffr/products/cup'];
    const rejected = [...invalidPurchases, ...wrongRoutes.map((buyerHref) => ({ buyerHref, variantId }))];
    for (const payload of rejected) {
      await setAction(page, 'one-action', { kind: 'emit_event', event: 'buy_now', payload });
      await page.getByTestId('one-action').click();
    }
    expect(page.url()).toBe('https://landing.test/page');
    const events = await page.evaluate(() => (window as unknown as BridgeTestWindow).bridgeEvents);
    if (host === 'Next') {
      expect(events).toHaveLength(rejected.length);
      expect(events.every((entry) => JSON.stringify(entry) === JSON.stringify(['one', 'error', 'buyer_handoff_invalid']))).toBe(true);
    } else {
      expect(events).toEqual([]);
      await expect(page.getByTestId('one-surface').locator('[data-fly-bridge-error="buyer_handoff_invalid"]')).toBeVisible();
      await expect(page.getByTestId('two-surface').locator('[data-fly-bridge-error="buyer_handoff_invalid"]')).toBeHidden();
    }
  });

  test(`${host} lets the canonical action own a cart-trigger click and ignores other providers`, async ({ page }) => {
    await mount(page);
    if (host === 'Next') await installNext(page);
    else {
      await page.addScriptTag({ content: await readFile(rustBridge, 'utf8') });
      await page.evaluate(() => {
        const state = window as unknown as BridgeTestWindow;
        state.bridgeEvents = [];
        window.addEventListener('rustok:cart:toggle', (event) => {
          event.preventDefault();
          state.bridgeEvents.push(['toggle']);
        });
      });
    }
    await setAction(page, 'one-cart', { kind: 'emit_event', event: 'cart:toggle' });
    await page.getByTestId('one-cart').click();
    expect(await page.evaluate(() => (window as unknown as BridgeTestWindow).bridgeEvents)).toEqual(
      host === 'Next' ? [['one', 'toggleCart']] : [['toggle']],
    );
    // A provider's operation called "cart:toggle" is not a host emit_event.
    await setAction(page, 'one-action', { kind: 'provider_action', provider: 'crm', action: 'cart:toggle', input: {} });
    await page.getByTestId('one-action').click();
    expect(await page.evaluate(() => (window as unknown as BridgeTestWindow).bridgeEvents)).toHaveLength(1);
  });
}

test('Leptos shows an unavailable cart until a host consumer acknowledges it', async ({ page }) => {
  await mount(page);
  await page.addScriptTag({ content: await readFile(rustBridge, 'utf8') });
  await page.getByTestId('one-cart').click();
  await expect(page.getByTestId('one-surface').locator('[data-fly-bridge-error="cart_unavailable"]')).toBeVisible();
  await expect(page.getByTestId('two-surface').locator('[data-fly-bridge-error="cart_unavailable"]')).toBeHidden();
  await expect(page.getByTestId('one-count')).toHaveText('—');
  await page.evaluate(() => window.addEventListener('rustok:cart:toggle', (event) => event.preventDefault()));
  await page.getByTestId('one-cart').click();
  await expect(page.getByTestId('one-surface').locator('[data-fly-bridge-error="cart_unavailable"]')).toBeHidden();
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('rustok:cart:updated', { detail: { count: -3 } })));
  await expect(page.getByTestId('one-count')).toHaveText('—');
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('rustok:cart:updated', { detail: { count: 7 } })));
  await expect(page.getByTestId('one-count')).toHaveText('7');
  // A second script include synchronizes newly rendered landings without resetting a known count.
  await page.addScriptTag({ content: await readFile(rustBridge, 'utf8') });
  await expect(page.getByTestId('two-count')).toHaveText('7');
  // Client navigation can insert a new landing without executing its inline script again.
  await page.evaluate(() => {
    const landing = document.createElement('div');
    landing.setAttribute('data-fly-page-body', 'true');
    landing.innerHTML = '<span data-fly-island="cart_count" data-testid="new-count">0</span>';
    document.body.append(landing);
  });
  await expect(page.getByTestId('new-count')).toHaveText('7');
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('rustok:cart:updated', { detail: { status: 'unknown' } })));
  await expect(page.getByTestId('new-count')).toHaveText('—');
  await expect(page.getByTestId('one-count')).toHaveText('—');
  await expect(page.getByTestId('two-count')).toHaveAttribute('data-fly-cart-count-status', 'unknown');
});

test('one published purchase payload hands off to Product on both hosts without a cart mutation', async ({ browser }) => {
  const context = await browser.newContext();
  try {
    const next = await context.newPage();
    const leptos = await context.newPage();
    await mount(next);
    await installNext(next);
    await mount(leptos);
    await leptos.addScriptTag({ content: await readFile(rustBridge, 'utf8') });
    await leptos.evaluate(() => window.addEventListener('rustok:cart:add', () => sessionStorage.setItem('unexpected-cart-add', '1')));
    const action = {
      kind: 'emit_event', event: 'buy_now',
      payload: { buyerHref: '/ru/products/cup?currency=AED#buyer', variantId: variantId.toUpperCase(), quantity: '3' },
    };
    await setAction(next, 'one-action', action);
    await setAction(leptos, 'one-action', action);
    await next.getByTestId('one-action').click();
    expect(await next.evaluate(() => (window as unknown as BridgeTestWindow).bridgeEvents)).toEqual([
      ['one', 'navigate', '/ru/products/cup?currency=AED&variant_id=11111111-1111-4111-8111-111111111111&quantity=3#buyer'],
    ]);
    await Promise.all([
      leptos.waitForURL('https://landing.test/ru/modules/products?handle=cup&currency=AED&variant_id=11111111-1111-4111-8111-111111111111&quantity=3#buyer'),
      leptos.getByTestId('one-action').click(),
    ]);
    expect(await leptos.evaluate(() => sessionStorage.getItem('unexpected-cart-add'))).toBeNull();
  } finally {
    await context.close();
  }
});
