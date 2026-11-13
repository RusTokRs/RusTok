/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

// Deterministic test boundary for the real Next production host. This is NOT the Rust server,
// an authorization test, or a substitute for exercising the real Product/Pricing/Cart services.
import { createServer } from 'node:http';

const port = Number(process.env.FLY_HOST_API_PORT || 3311);
if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error('Invalid FLY_HOST_API_PORT');

const variantId = '11111111-1111-4111-8111-111111111111';
const productId = '55555555-5555-4555-8555-555555555555';
const cartId = '22222222-2222-4222-8222-222222222222';
const sellerId = '77777777-7777-4777-8777-777777777777';
const requests = [];

const product = {
  id: productId, status: 'published', handle: 'cup', sellerId, vendor: null,
  productType: null, tags: [], publishedAt: null, images: [], attributes: [],
  translations: [{ locale: 'en', title: 'Smoke Cup', handle: 'cup', description: 'Test product' }],
  variantAxes: [],
  variants: [{
    id: variantId, title: 'Cup', sku: 'CUP', combinationIdentity: null, axisValues: [],
    inventoryPolicy: 'deny', inventoryQuantity: 20, inStock: true, prices: [],
  }],
};
const quote = {
  id: productId, resolvedAt: '2026-11-13T10:00:00Z',
  resolutionContext: {
    currencyCode: 'AED', regionId: null, priceListId: null,
    channelId: null, channelSlug: null, quantity: 2,
  },
  variants: [{
    id: variantId,
    effectivePrice: {
      currencyCode: 'AED', amount: '10.00', compareAtAmount: null, discountPercent: null,
      onSale: false, priceListId: null, channelId: null, channelSlug: null,
      source: { priceId: '66666666-6666-4666-8666-666666666666', kind: 'base', ruleKind: null, adjustmentPercent: null },
    },
  }],
};
function emptyCart() {
  return {
    id: cartId, currencyCode: 'AED', localeCode: 'en', regionId: null, channelId: null, channelSlug: null,
    status: 'active', subtotalAmount: '0', adjustmentTotal: '0', shippingTotal: '0', totalAmount: '0',
    taxTotal: '0', email: null, countryCode: null, selectedShippingOptionId: null,
    lineItems: [], adjustments: [], deliveryGroups: [],
  };
}
let cart = emptyCart();
const action = JSON.stringify({
  kind: 'emit_event', event: 'buy_now',
  payload: { buyerHref: '/en/products/cup?currency=AED', variantId, quantity: 2 },
});
const html = `<section data-testid="real-landing">
  <span data-fly-island="cart_count" data-testid="real-count">0</span>
  <button type="button" data-testid="real-cart" data-fly-island="cart_trigger">Cart</button>
  <button type="button" data-testid="real-toggle" data-fly-action='{"kind":"toggle_visibility","target_id":"panel"}'>Toggle</button>
  <div id="panel" data-fly-target-id="panel" data-testid="real-panel">Panel</div>
  <button type="button" data-testid="real-buy" data-fly-action='${action}'>Buy</button>
</section>`;

function send(res, status, data) {
  res.writeHead(status, { 'content-type': 'application/json', 'cache-control': 'no-store' });
  res.end(JSON.stringify(data));
}
createServer(async (req, res) => {
  if (req.url === '/__state' && req.method === 'GET') return send(res, 200, { requests, cart });
  if (req.url === '/__reset' && req.method === 'POST') {
    requests.length = 0;
    cart = emptyCart();
    return send(res, 200, { ok: true });
  }
  if (req.url?.startsWith('/api/seo/page-context')) return send(res, 200, null);
  if (req.url !== '/api/graphql' || req.method !== 'POST') return send(res, 404, { error: 'not found' });
  let input = '';
  for await (const chunk of req) input += chunk;
  let payload;
  try { payload = JSON.parse(input); } catch { return send(res, 400, { error: 'bad JSON' }); }
  const op = /\b(?:query|mutation)\s+(\w+)/.exec(payload.query ?? '')?.[1] ?? 'unknown';
  requests.push({ op, variables: payload.variables ?? null, tenant: req.headers['x-tenant-slug'] ?? null });
  if (op === 'EnabledModules') {
    return send(res, 200, { data: { enabledModules: ['pages', 'cart', 'product', 'commerce'] } });
  }
  if (op === 'StorefrontPage') {
    return send(res, 200, { data: { selectedPage: {
      id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', version: 1, status: 'published', effectiveLocale: 'en',
      translation: { locale: 'en', title: 'Smoke landing', slug: 'smoke', metaTitle: null, metaDescription: null },
      landing: { locale: 'en', html, css: '#panel { color: rgb(1, 2, 3); }', artifactHash: 'fake-test-hash' },
    } } });
  }
  if (op === 'StorefrontCommerceProduct') return send(res, 200, { data: { storefrontProduct: product } });
  if (op === 'StorefrontProductPricing') return send(res, 200, { data: { storefrontPricingProduct: quote } });
  if (op === 'StorefrontCart') return send(res, 200, { data: { storefrontCart: cart } });
  if (op === 'CreateStorefrontCart') return send(res, 200, { data: { createStorefrontCart: { cart } } });
  if (op === 'AddStorefrontCartLineItem') {
    const item = payload.variables?.input ?? {};
    cart = { ...cart, subtotalAmount: '20.00', totalAmount: '20.00', lineItems: [{
      id: '33333333-3333-4333-8333-333333333333', cartId, productId, variantId: item.variantId,
      sku: 'CUP', title: 'Cup', quantity: item.quantity, unitPrice: '10.00', totalPrice: '20.00',
      currencyCode: 'AED', shippingProfileSlug: null, sellerId,
    }] };
    return send(res, 200, { data: { addStorefrontCartLineItem: cart } });
  }
  return send(res, 200, { errors: [{ message: `Unexpected mock operation ${op}` }] });
}).listen(port, '0.0.0.0', () => console.log(`Fly Next test API listening on ${port}`));
