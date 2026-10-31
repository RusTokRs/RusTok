# apps/next-admin/packages/commerce / CRATE_API

## Purpose

`@rustok/commerce-admin` (`apps/next-admin/packages/commerce`) is the Next.js administration UI package for the `rustok-commerce` domain module. It provides full administrative interfaces for managing customer orders, multi-locale shipping profiles, promotional cart discount simulations, post-order changes (returns, exchanges, claims), and return resolution decisions.

## Architecture & Responsibilities

- **Module Registration:** Registers the commerce module with the host admin registry via `registerAdminModule` and supplies structured navigation items (`commerceNavItems`) under the `modulePlugins` group with `manager` role access guards.
- **Order Operations:** Exposes `OrdersTable` and column configurations with status badges (Draft, Confirmed, Paid, Shipped, Delivered, Cancelled), customer ID linking, line item counting, carrier tracking numbers, and actionable row menus.
- **Shipping Profile Management:** Provides `ShippingProfilesTemplate` supporting multi-locale translations (RU/EN), slug configuration, JSON metadata, and active/inactive toggle actions.
- **Cart Promotions Engine:** Exposes `CartPromotionsTemplate` for testing and simulating discount outcomes (PERCENT and AMOUNT rules at CART and LINE_ITEM scopes) with real-time preview and live cart application.
- **Order Changes & Returns Workflow:** Implements `OrderChangesTemplate` and `ReturnDecisionsTemplate` for processing lifecycle transitions on order returns, exchanges with price difference refunds, and warranty claims.
- **Direct GraphQL Transport:** Dispatches all operations through typed GraphQL queries (`shippingProfiles`, `orderChanges`, `orders`) and mutations (`createShippingProfile`, `previewAdminCartPromotion`, `applyAdminCartPromotion`, `createOrderReturnDecision`, `applyOrderChange`, `cancelOrderChange`).

## Public Surface

- `commerceNavItems`: Navigation configuration for the Next.js admin sidebar under `modulePlugins` (`access: { role: 'manager' }`).
- `OrdersTable`: TanStack table shell component displaying paginated orders with sortable, filterable columns and contextual row actions.
- `ShippingProfilesTemplate`: Full CRUD workspace component for shipping profile lifecycle management.
- `CartPromotionsTemplate`: Interactive workspace component for simulating and applying promo adjustments.
- `OrderChangesTemplate`: Inspection and review table for order change lifecycle transitions (pending, applied, cancelled).
- `ReturnDecisionsTemplate`: Form and result workspace for creating return-only, refund, exchange, or claim resolutions.
- `api/*`: Client functions and exported GraphQL documents for all commerce domain operations:
  - `listOrders`, `ORDERS_QUERY`
  - `listShippingProfiles`, `getShippingProfile`, `createShippingProfile`, `updateShippingProfile`, `deactivateShippingProfile`, `reactivateShippingProfile`, `SHIPPING_PROFILES_QUERY`, `SHIPPING_PROFILE_QUERY`, `CREATE_SHIPPING_PROFILE_MUTATION`, `UPDATE_SHIPPING_PROFILE_MUTATION`, `DEACTIVATE_SHIPPING_PROFILE_MUTATION`, `REACTIVATE_SHIPPING_PROFILE_MUTATION`
  - `previewCartPromotion`, `applyCartPromotion`, `PREVIEW_CART_PROMOTION_MUTATION`, `APPLY_CART_PROMOTION_MUTATION`
  - `listOrderChanges`, `getOrderChange`, `applyOrderChange`, `cancelOrderChange`, `ORDER_CHANGES_QUERY`, `ORDER_CHANGE_QUERY`, `APPLY_ORDER_CHANGE_MUTATION`, `CANCEL_ORDER_CHANGE_MUTATION`
  - `createOrderReturnDecision`, `CREATE_ORDER_RETURN_DECISION_MUTATION`
- `types/*`: Strongly typed TypeScript domain definitions for shipping profiles, promotions, order changes, returns, and order lists.

## Invariants & Guarantees

1. **Fail-Closed Auth & Tenant Isolation:** Every API function strictly validates `opts.token`, `opts.tenantSlug`, and `opts.tenantId`; missing credentials immediately throw an error before any network request.
2. **Schema Contract Hygiene:** All queries and mutations match backend `rustok-commerce` GraphQL definitions with exact field and variable typing.
3. **Role Guards:** Commerce navigation and administrative workspaces require the `manager` role at minimum.
4. **Input Validation:** Positive quantities are required for return decisions; required fields (slug, order ID, cart ID, source ID) are validated prior to mutation dispatch.
5. **Parity with Leptos Admin:** Commerce data models, order status badges, shipping profile translations, and return decision workflows maintain functional parity with `crates/modules/rustok-commerce/admin`.

## Dependencies & Boundaries

- **Host Interface:** Depends on React 19, `@/modules/registry`, `@/shared/ui/shadcn`, and `@/widgets/data-table` from `apps/next-admin`.
- **Backend API:** Communicates exclusively with `apps/server` via GraphQL endpoint (`/api/graphql`).
- **No Direct Database Access:** Never queries commerce tables or database entities directly.

