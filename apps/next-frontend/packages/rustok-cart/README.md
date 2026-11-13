# @rustok/cart-frontend

Frontend module package for the Storefront Shopping Cart in RusToK Next.js.

## Overview

`@rustok/cart-frontend` provides client-side cart management and presentation components:
- **`CartProvider` & `useCart()`**: Reactive Cart observation, tenant/actor-scoped local references and GraphQL mutations. The host's Cart capability and auth readiness/refusal participate in the committed asynchronous scope fence, including when the same actor returns after an interruption. Unmount invalidates captured actions; it does not cancel a transmitted request.
- **`CartDrawer`**: Shows confirmed line items, a pending state during reads/writes, or an explicit disabled/restore/unconfirmed/auth error with inspection and local reset where available; it never presents an unknown Cart as empty or stale totals as current while a mutation is pending.
- **`CartTrigger`**: Floating or inline control with a confirmed, pending or unknown count; it is disabled when the capability is off.
- **`CheckoutView`**: Does not present or start checkout while Cart is disabled, restoring, updating or unconfirmed. It does not fence or cancel a checkout already transmitted; see its remaining limitations below.

## GraphQL Contract

The package talks to backend GraphQL endpoints provided by `rustok-commerce`:
- **`storefrontCart(id: UUID!)`**: Fetch existing cart by ID.
- **`createStorefrontCart(input: CreateStorefrontCartInput!)`**: Create a new session cart.
- **`addStorefrontCartLineItem(cartId: UUID!, input: AddStorefrontCartLineItemInput!)`**: Add variant to cart.
- **`updateStorefrontCartLineItemQuantity(cartId: UUID!, lineItemId: UUID!, quantity: Int!)`**: Update item count.
- **`removeStorefrontCartLineItem(cartId: UUID!, lineItemId: UUID!)`**: Remove item from cart.

## Usage

```tsx
import { CartProvider, CartDrawer, CartTrigger } from "@rustok/cart-frontend";

export default function Layout({ children, locale, tenantSlug, auth, cartEnabled }) {
  return (
    <CartProvider locale={locale} tenantSlug={tenantSlug} enabled={cartEnabled}
      authToken={auth.token} authReady={auth.ready} authRefused={auth.refused}
      authScopeIsCurrent={auth.isCurrent}>
      {children}
      {cartEnabled && <><CartTrigger locale={locale} /><CartDrawer locale={locale} /></>}
    </CartProvider>
  );
}
```

The real host supplies these values from its shared auth and enabled-module contexts
(`src/widgets/providers/storefront-providers.tsx`); a landing must not invent them.

## Known Limitations / Pending Implementation

- A restore or inspection waits for known same-actor mutations in this JavaScript realm, including
  across provider remounts; an overlapping same-realm write also invalidates an older in-flight
  read. An unresolved persisted marker is not automatically cleared by a restore. A confirmed
  create that stopped before sending an add keeps its owner reference and clears its settled marker.
- Local mutation markers are uncertainty hints, not durable server receipts, cross-tab locks or an
  exactly-once protocol. They do not synchronize already rendered providers or other tabs: an
  uncoordinated write can start and finish while a read is in flight, leaving a stale observation.
  Inspection is only a snapshot and cannot prove that a lost request has finished; an explicit local
  reset does not undo or cancel it. A failed create without a confirmed reference cannot be
  inspected. The owner reference is preserved on failed reads.
- `CheckoutView` still synthesizes fallback shipping choices and uses floating-point presentation
  arithmetic. Checkout generates a new idempotency key per call (including retry) and lacks a
  durable recovery/scope-fencing protocol for uncertain completion. Unlike other Cart operations,
  `completeStorefrontCheckout` does not accept the provider's bearer token; signed-in completion
  needs an explicit owner/host authorization contract and live verification. These are pre-existing
  checkout gaps, not resolved by the Cart observation guard; do not treat the current UI as
  production-ready.
- The targeted React browser harness fakes router, translations and GraphQL. Real host navigation,
  authorization, checkout, and cross-client/deployment evidence remain outstanding.
