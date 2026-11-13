import { expect, test, type Page } from '@playwright/test';
import { readFile } from 'node:fs/promises';

// This simulated Leptos shell tests shipped scripts with the server's actual CSP template.
// It complements Rust SSR tests; it is not a deployed-host or hydration test.
const runtime = new URL('../../../../crates/ui/fly/browser/assets/fly-actions.js', import.meta.url);
const bridge = new URL('../../../../crates/modules/rustok-page-builder/storefront/assets/fly-storefront-bridge.js', import.meta.url);
const securityHeaders = new URL('../../../../apps/server/src/middleware/security_headers.rs', import.meta.url);
const nonce = 'test_nonce_123';

function policy(source: string, allowedNonce: string): string {
  const template = /const UI_CSP_TEMPLATE: &str =\s*"([^"]+)";/.exec(source)?.[1];
  const connect = /const SECURE_UI_CONNECT_SOURCES: &str =\s*"([^"]+)";/.exec(source)?.[1];
  if (!template || !connect || !template.includes('{nonce}') || !template.includes('{connect_sources}')) {
    throw new Error('Could not read the production UI CSP template and secure connection policy');
  }
  return template.replaceAll('{nonce}', `'nonce-${allowedNonce}'`).replaceAll('{connect_sources}', connect);
}

async function mount(page: Page, allowedNonce: string): Promise<void> {
  const [actions, storefrontBridge, cspSource] = await Promise.all([
    readFile(runtime, 'utf8'), readFile(bridge, 'utf8'), readFile(securityHeaders, 'utf8'),
  ]);
  const html = `<!doctype html><html lang="en"><head><meta charset="utf-8"></head><body>
    <div class="probe" data-testid="host-probe">Host</div>
    <section data-rustok-page-builder-storefront="true">
      <style data-fly-project-styles="true" nonce="${nonce}">
        section[data-rustok-page-builder-storefront]{isolation:isolate;contain:style layout}
        @scope ([data-fly-page-body]) { :scope { isolation: isolate; } .probe { display: none; color: rgb(1, 2, 3); } }
      </style>
      <div data-fly-page-body="true">
        <button type="button" data-testid="toggle" data-fly-action='{"kind":"toggle_visibility","target_id":"panel"}'>Toggle</button>
        <div id="panel" class="probe" data-testid="panel">Panel</div>
        <span data-fly-island="cart_count" data-testid="count">0</span>
        <button type="button" data-fly-island="cart_trigger" data-testid="cart">Cart</button>
        <button type="button" data-testid="bad-buy" data-fly-action='{"kind":"emit_event","event":"add_to_cart","payload":{"buyerHref":"javascript:alert(1)","variantId":"11111111-1111-4111-8111-111111111111"}}'>Buy</button>
        <script data-untrusted="true">window.untrustedRan = true;</script>
        <button type="button" data-testid="unsafe-attributes" onclick="window.handlerRan = true" style="color:rgb(50, 60, 70)">Unsafe attributes</button>
      </div>
      <p data-fly-bridge-error="buyer_handoff_invalid" role="alert" hidden>Invalid purchase link</p>
      <p data-fly-bridge-error="cart_unavailable" role="alert" hidden>Cart unavailable</p>
      <script data-fly-actions="true" nonce="${nonce}">${actions}</script>
      <script data-fly-storefront-bridge="true" nonce="${nonce}">${storefrontBridge}</script>
    </section>
  </body></html>`;
  await page.route('https://leptos-csp.test/**', (route) => route.fulfill({
    contentType: 'text/html',
    headers: { 'content-security-policy': policy(cspSource, allowedNonce), 'cache-control': 'private, no-store' },
    body: html,
  }));
  await page.goto('https://leptos-csp.test/en');
}

async function expectUntrustedMarkupBlocked(page: Page): Promise<void> {
  expect(await page.evaluate(() => (window as unknown as { untrustedRan?: boolean }).untrustedRan)).toBeUndefined();
  await page.getByTestId('unsafe-attributes').click();
  expect(await page.evaluate(() => (window as unknown as { handlerRan?: boolean }).handlerRan)).toBeUndefined();
  await expect(page.getByTestId('unsafe-attributes')).not.toHaveCSS('color', 'rgb(50, 60, 70)');
  await expect(page.getByTestId('host-probe')).not.toHaveCSS('color', 'rgb(1, 2, 3)');
}

test('matching CSP nonce allows landing CSS and runtime, not author script or inline attributes', async ({ page }) => {
  await mount(page, nonce);
  await expectUntrustedMarkupBlocked(page);
  await expect(page.getByTestId('panel')).toHaveCSS('color', 'rgb(1, 2, 3)');
  await expect(page.getByTestId('panel')).toBeHidden();
  await expect(page.getByTestId('count')).toHaveText('—');
  await expect(page.getByTestId('count')).toHaveAttribute('data-fly-cart-count-status', 'unknown');

  await page.getByTestId('toggle').click();
  await expect(page.getByTestId('panel')).toBeVisible();
  await page.getByTestId('toggle').click();
  await expect(page.getByTestId('panel')).toBeHidden();
  await page.getByTestId('cart').click();
  await expect(page.locator('[data-fly-bridge-error="cart_unavailable"]')).toBeVisible();
  await page.getByTestId('bad-buy').click();
  await expect(page.locator('[data-fly-bridge-error="buyer_handoff_invalid"]')).toBeVisible();
  expect(page.url()).toBe('https://leptos-csp.test/en');

  await page.evaluate(() => window.dispatchEvent(new CustomEvent('rustok:cart:updated', { detail: { count: 3 } })));
  await expect(page.getByTestId('count')).toHaveText('3');
  await page.evaluate(() => {
    const counter = document.createElement('span');
    counter.setAttribute('data-fly-island', 'cart_count');
    counter.setAttribute('data-testid', 'new-count');
    counter.textContent = '0';
    document.querySelector('[data-fly-page-body]')!.appendChild(counter);
  });
  await expect(page.getByTestId('new-count')).toHaveText('3');
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('rustok:cart:updated', { detail: { status: 'unknown' } })));
  await expect(page.getByTestId('new-count')).toHaveText('—');
});

test('a mismatched CSP nonce blocks the landing CSS and both scripts', async ({ page }) => {
  await mount(page, 'wrong_nonce');
  await expectUntrustedMarkupBlocked(page);
  await expect(page.getByTestId('panel')).not.toHaveCSS('color', 'rgb(1, 2, 3)');
  await expect(page.getByTestId('count')).toHaveText('0');
  await page.getByTestId('toggle').click();
  await expect(page.getByTestId('panel')).toBeVisible();
  await page.getByTestId('cart').click();
  await expect(page.locator('[data-fly-bridge-error="cart_unavailable"]')).toBeHidden();
});
