# @rustok/order-frontend / CRATE_API

## 1. Public Interfaces & Architecture

`@rustok/order-frontend` (`apps/next-frontend/packages/rustok-order`) is the Next.js storefront customer order history and tracking package for RusToK. It manages single order lookup, live status tracking, printable receipts, and customer order history in full parity with `crates/modules/rustok-commerce/storefront` (Leptos).

### Public Surface (`src/index.tsx`)
- **Components:** `OrderView`, `OrdersHistoryView`.
- **API Callers:**
  - `fetchStorefrontOrder(graphql, orderId, tenantSlug, token): Promise<Order | null>`
  - `fetchStorefrontOrders(graphql, params, tenantSlug, token): Promise<OrderListResponse | null>`
  - `getLocalRecentOrderIds(): string[]`
  - `saveLocalRecentOrderId(orderId: string): void`
  - `fetchGuestRecentOrders(graphql, orderIds, tenantSlug): Promise<Order[]>`
- **Types:** `Order`, `OrderLineItem`, `OrderAdjustment`, `OrdersFilterParams`, `OrderListResponse`.

---

## 2. Key Contracts, Routes & Layouts

### Data Contracts
- `Order`: ID, status (`PENDING`, `CONFIRMED`, `PAID`, `PROCESSING`, `SHIPPED`, `DELIVERED`, `CANCELLED`), currencyCode, exact decimal strings (`subtotalAmount`, `adjustmentTotal`, `shippingTotal`, `totalAmount`, `taxTotal`), `lineItems`, `adjustments`, carrier, tracking number, and lifecycle timestamps (`confirmedAt`, `paidAt`, `shippedAt`, `deliveredAt`, `cancelledAt`).
- `OrderLineItem`: ID, title, quantity, exact decimal strings (`unitPrice`, `totalPrice`), and currency.

### Lifecycle Tracking (`OrderView`)
- 5-stage progress indicator: *Placed → Paid → Processing → Shipped → Delivered*.
- Displays tracking number with one-click copy and carrier identification.
- Guest customer recent orders stored in `localStorage` under `rustok_customer_recent_orders` (capped at 25 entries).

---

## 3. Integration, Transport & Security Boundary

### Transport Boundary
- Dispatches typed GraphQL queries (`StorefrontOrder`, `StorefrontOrders`) through shared `storefrontGraphql`.
- Propagates Bearer customer session tokens and `X-Tenant-Slug` headers.
- Guest order lookups require explicit UUID string without leaking private customer fields.

---

## 4. Verification & Testing Evidence

- **Unit Tests**: `order-frontend.test.mjs` verifies GraphQL query schemas, recent order ID deduplication, and max 25 entry bounds.
- **Storefront Boundary Gate**: `npm run verify:order:storefront-boundary` confirms bounded error shapes and clean UI envelopes.
- **Next.js Host Typecheck**: `npm run typecheck:next-frontend` completes with 0 errors.

---

## 5. Anti-Patterns & Operational Invariants

1. **No Floating-Point Calculations**: Monetary sums and adjustments must strictly remain exact decimal strings.
2. **Cap Guest Order Storage**: `localStorage` recent order history is strictly capped at 25 items to prevent unbounded storage growth.
3. **Fail-Closed Guest Queries**: Empty or whitespace-only order IDs must be rejected before making GraphQL requests.

