# @rustok/cart-frontend / CRATE_API

## 1. Public Interfaces & Architecture

`@rustok/cart-frontend` (`apps/next-frontend/packages/rustok-cart`) is the Next.js storefront shopping cart and checkout package for RusToK. It manages client-side cart state, line item modifications, delivery group shipping selection, and order completion workflows in full parity with `crates/modules/rustok-cart/storefront` (Leptos).

### Public Surface (`src/index.tsx`)
- **Components:** `CartDrawer`, `CartTrigger`, `CheckoutView`.
- **Context & Hooks:** `CartProvider`, `useCart`, `CartContextValue`.
- **API Callers:**
  - `fetchStorefrontCart(graphql, cartId, tenantSlug): Promise<Cart | null>`
  - `createStorefrontCart(graphql, input, tenantSlug): Promise<Cart | null>`
  - `addStorefrontCartLineItem(graphql, cartId, variantId, quantity, tenantSlug): Promise<Cart | null>`
  - `updateStorefrontCartLineItemQuantity(graphql, cartId, lineItemId, quantity, tenantSlug): Promise<Cart | null>`
  - `removeStorefrontCartLineItem(graphql, cartId, lineItemId, tenantSlug): Promise<Cart | null>`
  - `updateStorefrontCartShipping(graphql, cartId, shippingOptionId, shippingSelections, tenantSlug): Promise<Cart | null>`
  - `completeStorefrontCheckout(graphql, input, tenantSlug): Promise<CompleteCheckoutResult | null>`
- **Types:** `Cart`, `CartLineItem`, `CartAdjustment`, `CartDeliveryGroup`, `CartShippingOptionSummary`, `CompleteStorefrontCheckoutInput`, `OrderSummary`, `PaymentCollectionSummary`.

---

## 2. Key Contracts, Routes & Layouts

### Data Contracts
- `Cart`: Unique ID, status (`ACTIVE`, `COMPLETED`, `ABANDONED`), exact decimal amounts (`subtotalAmount`, `adjustmentTotal`, `shippingTotal`, `totalAmount`, `taxTotal`), `lineItems`, `adjustments`, and `deliveryGroups`.
- `CartLineItem`: Unique ID, parent `cartId`, optional `productId`/`variantId`/`sku`, `title`, integer `quantity`, exact decimal string `unitPrice` and `totalPrice`, and `currencyCode`.
- `CartDeliveryGroup`: Grouping by `shippingProfileSlug` and optional `sellerId`, holding `selectedShippingOptionId` and array of `availableShippingOptions`.

### Context Lifecycle (`CartProvider`)
- Restores active cart ID from `window.localStorage` key `rustok_storefront_cart_id`.
- Automatically evicts stale or non-`ACTIVE` carts from storage.
- Auto-creates an active cart upon the first `addItem` invocation.
- Aggregates `itemCount` reactively from `lineItems.reduce((acc, item) => acc + item.quantity, 0)`.

---

## 3. Integration, Transport & Security Boundary

### Transport Boundary
- Dispatches typed GraphQL operations over the shared storefront transport (`storefrontGraphql`).
- Generates secure random RFC 4122 v4 idempotency keys (`crypto.randomUUID()`) for `completeStorefrontCheckout` mutations to prevent double-billing.
- Propagates tenant context via `X-Tenant-Slug` on all cart queries and mutations.

### Error Handling & Invariants
- `quantity <= 0` in `updateQuantity` automatically routes to `removeStorefrontCartLineItem`.
- `completeStorefrontCheckout` re-throws server and transport errors so UI can render contextual error notices instead of hanging or claiming success.

---

## 4. Verification & Testing Evidence

- **Unit Tests**: `cart-frontend.test.mjs` verifies item quantity accumulation, line item quantity edge cases, and zero-removal transitions.
- **Storefront Boundary Gate**: `npm run verify:cart:storefront-boundary` confirms bounded SSR diagnostics and clean UI separation.
- **GraphQL Error Safety**: `node scripts/verify/verify-cart-storefront-graphql-error-safety.mjs` verifies error preservation.
- **Next.js Host Typecheck**: `npm run typecheck:next-frontend` completes with 0 errors.

---

## 5. Anti-Patterns & Operational Invariants

1. **No Floating-Point Currency Arithmetic**: All price and adjustment amounts are treated as exact major-unit decimal strings.
2. **Purge Inactive Storage**: Expired, completed, or missing cart IDs must be deleted from browser local storage immediately upon restoration failure.
3. **Idempotent Checkout Execution**: Checkout completion must always include a unique caller idempotency key.
4. **Zero Quantity Deletion**: Never dispatch negative or zero quantity updates; line items must be removed explicitly.
