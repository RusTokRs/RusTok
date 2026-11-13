/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

import { useCallback, useLayoutEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { CartProvider, useCart, type CartContextValue } from '../../../packages/rustok-cart/src/context/cart-context';
import { CartDrawer } from '../../../packages/rustok-cart/src/components/cart-drawer';
import { CartTrigger } from '../../../packages/rustok-cart/src/components/cart-trigger';
import { CheckoutView } from '../../../packages/rustok-cart/src/components/checkout-view';
import type { Cart } from '../../../packages/rustok-cart/src/api/types';
import { PageBuilderContent } from '../../../packages/rustok-pages/src/components/page-builder-content';
import { ProductBuyerPanel } from '../../../packages/rustok-product/src/components/product-buyer-panel';
import { parseBuyerRoute } from '../../../packages/rustok-product/src/catalog/buyer';
import type { ProductGraphqlExecutor } from '../../../packages/rustok-product/src/api/products';
import type { StorefrontProductDetail, StorefrontProductPricingDetail } from '../../../packages/rustok-product/src/api/types';
import { storefrontGraphql } from './fly-react-host';
import { EnabledModulesClientProvider, useEnabledModulesContext } from '../../../src/shared/lib/enabled-modules-context';

declare global {
  interface Window {
    flyHarness: {
      requests: string[];
      navigation: string[];
      cart: CartContextValue | null;
      secondCart: CartContextValue | null;
      resolveCreate: (() => void) | null;
      resolveUpdate: (() => void) | null;
      resolveRead: (() => void) | null;
      deferRead: boolean;
      failRead: boolean;
      failUpdateResponse: boolean;
      addResult: boolean | null;
      updateResult: boolean | null;
      ownerCart: Cart;
      productQuote: StorefrontProductPricingDetail;
      deferQuote: boolean;
      quoteResolvers: Array<(quote: StorefrontProductPricingDetail) => void>;
      addVariables: unknown;
      providerCommits: string[];
      clickCartOnDisable: boolean;
      counterAtDisable: string | null;
      staleToggle: (() => void) | null;
      staleAdd: CartContextValue['addItem'] | null;
      startStaleAddOnDisable: boolean;
      unmount: (() => void) | null;
      remount: (() => void) | null;
    };
  }
}

const variantId = '11111111-1111-4111-8111-111111111111';
const product: StorefrontProductDetail = {
  id: '55555555-5555-4555-8555-555555555555', status: 'published', handle: 'cup',
  sellerId: 'platform', tags: [], images: [], translations: [], variantAxes: [],
  variants: [{
    id: variantId, title: 'Cup', sku: 'CUP', inventoryQuantity: 20, inventoryPolicy: 'deny',
    combinationIdentity: null, axisValues: [], inStock: true, prices: [],
  }],
};
const productQuote: StorefrontProductPricingDetail = {
  id: product.id, resolvedAt: '2026-11-13T10:00:00Z',
  resolutionContext: { currencyCode: 'AED', regionId: null, priceListId: null, channelId: null, channelSlug: null, quantity: 2 },
  variants: [{ id: variantId, effectivePrice: {
    currencyCode: 'AED', amount: '10.00', compareAtAmount: null, onSale: false,
    priceListId: null, channelId: null, channelSlug: null,
    source: { priceId: '66666666-6666-4666-8666-666666666666', kind: 'base', ruleKind: null, adjustmentPercent: null },
  } }],
};

window.flyHarness = {
  requests: [], navigation: [], cart: null, secondCart: null, resolveCreate: null, resolveUpdate: null,
  resolveRead: null, deferRead: false, failRead: false, failUpdateResponse: false,
  addResult: null, updateResult: null,
  productQuote, deferQuote: false, quoteResolvers: [], addVariables: null,
  providerCommits: [], clickCartOnDisable: false, counterAtDisable: null, staleToggle: null,
  staleAdd: null, startStaleAddOnDisable: false, unmount: null, remount: null,
  ownerCart: {
    id: '22222222-2222-4222-8222-222222222222', currencyCode: 'AED', localeCode: 'ru',
    regionId: null, channelId: null, channelSlug: null, status: 'active',
    subtotalAmount: '0', adjustmentTotal: '0', shippingTotal: '0', totalAmount: '0', taxTotal: '0',
    lineItems: [], adjustments: [],
  },
};

function Probe() {
  const cart = useCart();
  useLayoutEffect(() => {
    window.flyHarness.cart = cart;
    if (!cart.enabled && window.flyHarness.startStaleAddOnDisable) {
      window.flyHarness.startStaleAddOnDisable = false;
      void window.flyHarness.staleAdd?.(variantId, 2, {
        locale: 'ru', currencyCode: 'AED', regionId: null, priceListId: null,
        channelId: null, channelSlug: null, quantity: 2,
      }).then((value) => { window.flyHarness.addResult = value; });
    }
  }, [cart]);
  return <div data-testid="probe" data-enabled={String(cart.enabled)} data-loading={String(cart.isLoading)}
    data-updating={String(cart.isUpdating)} data-open={String(cart.isOpen)} data-reference={cart.reference ?? ''} />;
}

function SecondCartProbe() {
  const cart = useCart();
  useLayoutEffect(() => { window.flyHarness.secondCart = cart; }, [cart]);
  return <div data-testid="second-probe" data-loading={String(cart.isLoading)}
    data-updating={String(cart.isUpdating)} data-error={cart.actionError ?? ''} data-count={String(cart.itemCount)} />;
}

function ModuleProbe({ serverEnabled, refresh }: { serverEnabled: boolean; refresh: number }) {
  const { enabledModules, setModuleEnabled } = useEnabledModulesContext();
  const enabled = enabledModules.includes('cart');
  useLayoutEffect(() => {
    window.flyHarness.providerCommits.push(`${serverEnabled}:${refresh}:${enabled}`);
  }, [serverEnabled, refresh, enabled]);
  return <div data-testid="module-probe" data-enabled={String(enabled)}>
    <button data-testid="module-override" onClick={() => setModuleEnabled('cart', !enabled)}>Override</button>
  </div>;
}

function ModuleProviderHarness() {
  const [server, setServer] = useState({ enabled: true, refresh: 0, modules: ['cart'] });
  return <>
    <button data-testid="server-capability" onClick={() => setServer((prev) => {
      const enabled = !prev.enabled;
      return { ...prev, enabled, modules: enabled ? ['cart'] : [] };
    })}>Server capability</button>
    <button data-testid="refresh-server-modules" onClick={() => setServer((prev) => ({
      ...prev, refresh: prev.refresh + 1, modules: [...prev.modules],
    }))}>Refresh server modules</button>
    <EnabledModulesClientProvider initialModules={server.modules}>
      <ModuleProbe serverEnabled={server.enabled} refresh={server.refresh} />
    </EnabledModulesClientProvider>
  </>;
}

const action = JSON.stringify({
  kind: 'emit_event', event: 'buy_now',
  payload: { buyerHref: '/ru/products/cup?currency=AED', variantId: '11111111-1111-4111-8111-111111111111', quantity: 2 },
});
const html = `<span data-fly-island="cart_count" data-testid="count">0</span>
  <button data-fly-island="cart_trigger" data-testid="cart">Cart</button>
  <button data-testid="buy" data-fly-action='${action}'>Buy</button>
  <button data-testid="invalid" data-fly-action='{"kind":"emit_event","event":"buy_now","payload":{"productId":"invalid"}}'>Invalid</button>`;

function App() {
  const [enabled, setEnabled] = useState(false);
  const [authAvailable, setAuthAvailable] = useState(true);
  const isCurrent = useCallback(() => authAvailable, [authAvailable]);
  const [replacement, setReplacement] = useState(false);
  const [checkout, setCheckout] = useState(false);
  const [buyer, setBuyer] = useState(false);
  const [showSecond, setShowSecond] = useState(false);
  useLayoutEffect(() => {
    if (!enabled && window.flyHarness.clickCartOnDisable) {
      window.flyHarness.clickCartOnDisable = false;
      window.flyHarness.counterAtDisable = document.querySelector('[data-testid="count"]')?.textContent ?? null;
      (document.querySelector('[data-testid="cart"]') as HTMLElement | null)?.click();
    }
  }, [enabled]);
  // The fake router records the landing handoff; the real Product parser consumes its query.
  const buyerHref = window.flyHarness.navigation.at(-1) ?? '/ru/products/cup?currency=AED&variant_id=11111111-1111-4111-8111-111111111111&quantity=2';
  const buyerUrl = new URL(buyerHref, window.location.origin);
  const buyerParams = Object.fromEntries(buyerUrl.searchParams);
  const initialRoute = parseBuyerRoute(product, buyerParams, 'ru');
  return <CartProvider locale="ru" tenantSlug="acme" authReady={authAvailable} authRefused={!authAvailable} authScopeIsCurrent={isCurrent} enabled={enabled}>
    <ModuleProviderHarness />
    <button data-testid="capability" onClick={() => setEnabled((value) => !value)}>Capability</button>
    <button data-testid="auth" onClick={() => setAuthAvailable((value) => !value)}>Auth availability</button>
    <button data-testid="replace-landing" onClick={() => setReplacement(true)}>Replace landing</button>
    <button data-testid="show-checkout" onClick={() => setCheckout(true)}>Show checkout</button>
    <button data-testid="show-buyer" onClick={() => setBuyer(true)}>Show Product buyer</button>
    <button data-testid="show-second-cart" onClick={() => setShowSecond(true)}>Show second Cart</button>
    <Probe />
    {showSecond && <CartProvider locale="ru" tenantSlug="acme" authReady={authAvailable}
      authRefused={!authAvailable} authScopeIsCurrent={isCurrent} enabled={enabled}>
      <SecondCartProbe />
    </CartProvider>}
    <PageBuilderContent html={replacement
      ? '<span data-fly-island="cart_count" data-testid="replacement-count">0</span>' : html} css="" />
    {checkout && <div data-testid="checkout"><CheckoutView locale="ru" tenantSlug="acme" /></div>}
    {buyer && <div data-testid="buyer"><ProductBuyerPanel
      product={product} handle="cup" locale="ru" tenantSlug="acme"
      graphql={storefrontGraphql as unknown as ProductGraphqlExecutor}
      path="/ru/products/cup" initialSearch={buyerUrl.search.slice(1)}
      initialRoute={initialRoute} initialQuote={productQuote}
    /></div>}
    {/* Keep the real components mounted across capability changes to test their own guards;
        the production host normally unmounts them while Cart is disabled. */}
    <CartTrigger locale="ru" variant="inline" /><CartDrawer locale="ru" />
  </CartProvider>;
}
const root = document.getElementById('app');
if (!root) throw new Error('Missing test mount');
let reactRoot = createRoot(root);
window.flyHarness.unmount = () => reactRoot.unmount();
window.flyHarness.remount = () => {
  reactRoot.unmount();
  reactRoot = createRoot(root);
  reactRoot.render(<App />);
};
reactRoot.render(<App />);
