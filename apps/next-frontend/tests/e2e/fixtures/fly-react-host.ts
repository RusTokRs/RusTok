/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

// Only external host capabilities are faked; CartProvider and PageBuilderContent are real.
const router = { push: (href: string) => window.flyHarness.navigation.push(href) };
export function useRouter() { return router; }
export function useTranslations() { return (key: string) => key; }

export async function storefrontGraphql(request: { query: string; variables?: unknown }) {
  const harness = window.flyHarness;
  harness.requests.push(request.query);
  if (request.query.includes('query StorefrontProductPricing')) {
    if (harness.deferQuote) {
      return new Promise((resolve) => {
        harness.quoteResolvers.push((quote) => resolve({ data: { storefrontPricingProduct: quote } }));
      });
    }
    return { data: { storefrontPricingProduct: harness.productQuote } };
  }
  if (request.query.includes('AddStorefrontCartLineItem')) {
    const variables = request.variables as { cartId: string; input: { variantId: string; quantity: number } };
    harness.addVariables = variables;
    harness.ownerCart = {
      ...harness.ownerCart,
      lineItems: [{
        id: '33333333-3333-4333-8333-333333333333', cartId: variables.cartId,
        variantId: variables.input.variantId, title: 'Cup', quantity: variables.input.quantity,
        unitPrice: '10.00', totalPrice: '20.00', currencyCode: 'AED',
      }],
    };
    return { data: { addStorefrontCartLineItem: harness.ownerCart } };
  }
  if (request.query.includes('CreateStorefrontCart')) {
    return new Promise((resolve) => {
      harness.resolveCreate = () => resolve({ data: { createStorefrontCart: { cart: harness.ownerCart } } });
    });
  }
  if (request.query.includes('UpdateStorefrontCartLineItem')) {
    const variables = request.variables as { lineId: string; input: { quantity: number } };
    return new Promise((resolve, reject) => {
      harness.resolveUpdate = () => {
        harness.ownerCart = {
          ...harness.ownerCart,
          lineItems: harness.ownerCart.lineItems.map((line) => line.id === variables.lineId
            ? { ...line, quantity: variables.input.quantity } : line),
        };
        if (harness.failUpdateResponse) reject(new Error('Response lost after update'));
        else resolve({ data: { updateStorefrontCartLineItem: harness.ownerCart } });
      };
    });
  }
  if (request.query.includes('query StorefrontCart')) {
    if (harness.failRead) throw new Error('Cart read unavailable in test host');
    if (harness.deferRead) {
      // Preserve the owner's observation at request time so delayed stale-read races are visible.
      const snapshot = structuredClone(harness.ownerCart);
      return new Promise((resolve) => {
        harness.resolveRead = () => resolve({ data: { storefrontCart: snapshot } });
      });
    }
    return { data: { storefrontCart: harness.ownerCart } };
  }
  throw new Error('Unexpected Cart operation in the test host');
}
