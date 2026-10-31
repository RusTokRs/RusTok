# @rustok/commerce-admin

Next.js admin UI package for the `rustok-commerce` module.

## Responsibilities

- Exposes commerce operational workspaces for `apps/next-admin`:
  - **Orders Management**: Order list with status filters, tracking details, and detail views (`OrdersTable`).
  - **Shipping Profiles**: CRUD management and activation controls for multi-locale shipping profiles (`ShippingProfilesTemplate`).
  - **Cart Promotions**: Interactive promotion simulator, preview calculation, and direct cart application (`CartPromotionsTemplate`).
  - **Order Changes**: Workflow management for order returns, exchanges, and claim approvals (`OrderChangesTemplate`).
  - **Return Decisions**: Orchestration form for return-only, refund, exchange, and claim resolution paths (`ReturnDecisionsTemplate`).
- Communicates directly with the RusToK GraphQL API via typed queries and mutations without intermediate server-side proxies.
- Registers navigation items under the `modulePlugins` group with manager role access guards.

