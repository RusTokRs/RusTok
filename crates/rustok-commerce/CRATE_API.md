# rustok-commerce / CRATE_API

## Public modules

`controllers`, `dto`, `entities`, `error`, `graphql`, `services`, `state_machine`.

## Primary public types and functions

- `pub struct CommerceModule`
- `pub struct CatalogService`, `pub struct InventoryService`, `pub struct PricingService`
- `pub struct CommerceQuery`, `pub struct CommerceMutation`
- `pub fn controllers::routes() -> Routes`
- `pub struct Order<S>` with states `Pending`, `Confirmed`, `Paid`, `Shipped`, `Delivered`, `Cancelled`
- `pub enum CommerceError`, `pub type CommerceResult<T>`

## Split boundary

- `dto`, `entities`, `error`, and search helpers are re-exported from `rustok-commerce-foundation`.
- `CatalogService` is re-exported from `rustok-product`.
- `PricingService` is re-exported from `rustok-pricing`.
- `InventoryService` is re-exported from `rustok-inventory`.
- `graphql`, `controllers`, and `state_machine` remain in `rustok-commerce` as the legacy compatibility
  and transport/orchestration facade of the ecommerce family.
- `migrations()` exposes only umbrella-owned migrations that still remain in `rustok-commerce`.
  Product, pricing, and inventory migrations stay owned by their dedicated submodules.
- `ProductResponse` now keeps backward-compatible flat fields and also returns translation groups for
  product options, variant titles, and image alt text when the normalized translation tables are populated.

## Events

- Publishes commerce domain events through the extracted services and outbox flow.
- Does not subscribe directly to external events in this crate.

## Dependencies on other RusToK crates

- `rustok-core`
- `rustok-api`
- `rustok-commerce-foundation`
- `rustok-product`
- `rustok-pricing`
- `rustok-inventory`
- `rustok-events`
- `rustok-outbox`
- (dev) `rustok-test-utils`

## Common mistakes

- Re-introducing product, pricing, or inventory business logic back into `rustok-commerce` instead of the
  dedicated split module.
- Treating `rustok-commerce` as a low-level shared dependency of its own submodules. It is the umbrella/root
  module of the family, not the bottom layer.
- Changing order status outside the state machine.
- Bypassing `ValidateEvent` or the transactional outbox when publishing events.
- Moving transport adapters back into `apps/server` instead of extending
  `crates/rustok-commerce/src/graphql/*` or `crates/rustok-commerce/src/controllers/*`.

## Minimal contract surface

### Input DTOs and commands

- Public DTOs and command inputs are exported through this crate, even when implemented in
  `rustok-commerce-foundation`.
- Changes to public DTO fields are breaking changes and require synchronized updates in transport adapters.
- GraphQL and HTTP entry points remain part of the crate's public API.

### Domain invariants

- Domain invariants remain enforced by services, DTO validation, and the order state machine.
- Multi-tenant boundaries, permission checks, and tenant-scoped queries remain mandatory.

### Events and outbox side effects

- Domain events must keep using the transactional outbox flow.
- Event payloads and event types must remain backward compatible for downstream consumers.

### Errors and failure codes

- `CommerceError` and `CommerceResult<T>` define the public failure contract of the crate.
- Validation, auth, conflict, and not-found scenarios must preserve stable error semantics across
  HTTP, GraphQL, and internal callers.
