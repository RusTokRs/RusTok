# @rustok/cart-frontend / CRATE_API

## 1. Public Interfaces & Architecture

`@rustok/cart-frontend` (`apps/next-frontend/packages/rustok-cart`) is the Next.js storefront Cart client. It manages client-side Cart observations, line item mutations, shipping selection and checkout presentation. It does not establish full runtime parity with the Leptos Cart or prove checkout readiness; see [Known Limitations](README.md#known-limitations--pending-implementation).

### Public Surface (`src/index.tsx`)
- **Components:** `CartDrawer`, `CartTrigger`, `CheckoutView`.
- **Context & Hooks:** `CartProvider`, `useCart`, `CartContextValue`.
- **API Callers:**
  - `fetchStorefrontCart(graphql, cartId, tenantSlug?, token?): Promise<Cart | null>`
  - `createStorefrontCart(graphql, input?, tenantSlug?, token?): Promise<Cart | null>`
  - `addStorefrontCartLineItem(graphql, cartId, variantId, quantity, tenantSlug, expectedContext, token?): Promise<Cart | null>` — requires the Product/Pricing observation; it does not invent one.
  - `updateStorefrontCartLineItemQuantity(graphql, cartId, lineItemId, quantity, tenantSlug?, token?): Promise<Cart | null>`
  - `removeStorefrontCartLineItem(graphql, cartId, lineItemId, tenantSlug?, token?): Promise<Cart | null>`
  - `updateStorefrontCartShipping(graphql, cartId, shippingOptionId, shippingSelections?, tenantSlug?, token?): Promise<Cart | null>`
  - `completeStorefrontCheckout(graphql, input, tenantSlug?): Promise<CompleteCheckoutResult | null>` — unlike the Cart operations above, it accepts no bearer token.
- **Types:** `Cart`, `CartLineItem`, `CartAdjustment`, `CartDeliveryGroup`, `CartShippingOptionSummary`, `CompleteStorefrontCheckoutInput`, `OrderSummary`, `PaymentCollectionSummary`.

---

## 2. Key Contracts, Routes & Layouts

### Data Contracts
- `Cart`: Unique ID, owner status string (`active` is required for adoption), decimal-string amounts (`subtotalAmount`, `adjustmentTotal`, `shippingTotal`, `totalAmount`, `taxTotal`), `lineItems`, `adjustments`, and optional `deliveryGroups`.
- `CartLineItem`: Unique ID, parent `cartId`, optional `productId`/`variantId`/`sku`, `title`, integer `quantity`, exact decimal string `unitPrice` and `totalPrice`, and `currencyCode`.
- `CartDeliveryGroup`: Grouping by `shippingProfileSlug` and optional `sellerId`, holding `selectedShippingOptionId` and array of `availableShippingOptions`.

### Context Lifecycle (`CartProvider`)
- Restores a tenant/actor-scoped local reference through an owner read; a legacy global ID is considered only on initial adoption. Failed reads and inactive references are retained for inspection or explicit local reset, not reinterpreted as empty.
- Capability, auth readiness/refusal, tenant, actor and locale changes fence Cart asynchronous work, including an interruption and return to the same actor. The committed fence precedes descendant layout effects and is invalidated on unmount. A restore or inspection waits for known same-actor mutations in the same JavaScript realm, including across provider remounts; a revision detects a same-realm mutation that completes during a read. This does not synchronize other tabs or already-rendered provider projections, or fence checkout completion once submitted.
- On an explicitly validated first add, Cart may be created; the SKU and `CartPricingExpectation` must come from Product/Pricing. An unresolved persisted mutation marker is not automatically cleared by restore. Unknown outcomes block automatic retry and are exposed to the UI; inspection is a snapshot, not a server receipt or proof of cancellation.
- `itemCount` derives from the confirmed cart's line items. The trigger, drawer and checkout distinguish loading, unknown and confirmed states.

---

## 3. Integration, Transport & Security Boundary

### Transport Boundary
- Dispatches typed GraphQL operations over the shared storefront transport (`storefrontGraphql`).
- `completeStorefrontCheckout` creates a fresh key per call, using `crypto.randomUUID()` where available and a non-cryptographic fallback otherwise. It does not retain a key across retries, so this is not a durable exactly-once or double-billing guarantee.
- Passes tenant context to the executor on Cart operations and passes the scoped bearer token on Cart reads/writes when present. Checkout completion accepts no bearer token; its signed-in authorization path still needs owner/host verification.

### Error Handling & Invariants
- `quantity <= 0` in `updateQuantity` automatically routes to `removeStorefrontCartLineItem`.
- `completeStorefrontCheckout` re-throws server and transport errors so UI can render contextual error notices instead of hanging or claiming success.

---

## 4. Verification & Testing Evidence

- **Unit Tests**: `cart-frontend.test.mjs` verifies item quantity accumulation, line item quantity edge cases, and zero-removal transitions.
- **Storefront Boundary Gate**: `npm run verify:cart:storefront-boundary` confirms bounded SSR diagnostics and clean UI separation.
- **GraphQL Error Safety**: `node scripts/verify/verify-cart-storefront-graphql-error-safety.mjs` verifies error preservation.
- **Next.js Host Typecheck**: `npm --prefix apps/next-frontend run typecheck`.
- **Targeted browser harness**: `npm --prefix apps/next-frontend run test:e2e:fly` exercises the real Cart provider, trigger, drawer and checkout guard with fake GraphQL/router/translation boundaries, including same-actor auth and capability A→B→A transitions, commit-time callbacks, unmount/remount, unknown markers and overlapping same-realm reads. It is not a live checkout, backend, cross-tab or deployed-host test.

---

## 5. Anti-Patterns & Operational Invariants

1. **Owner data**: Amounts arrive as decimal strings, but the existing drawer/checkout presentation uses floating-point arithmetic. The latter is a known gap, not an exact-money guarantee.
2. **Preserve uncertainty**: A failed read or unconfirmed mutation is not proof of an empty Cart. Keep the reference/marker for inspection or an explicit local reset.
3. **Checkout recovery**: A newly generated key per attempt is not retry-safe idempotency; durable completion recovery remains open.
4. **Zero Quantity Deletion**: `updateQuantity` routes a non-positive quantity to removal rather than sending a negative quantity update.
