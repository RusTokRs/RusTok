# rustok-order-admin / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA UI components for the order administration dashboard.
- Integrates with `rustok-grid-leptos`, `rustok-ui-core`, `rustok-ui-routing`, and `rustok-ui-i18n`.

## Domain Collaborations
- Collaborates with `rustok-order` / `rustok-commerce` for order queries, details, and lifecycle mutations.
- Collaborates with `apps/admin` as a registered admin module package.

## Public surface
- `pub mod core`: Pure Rust view-models, filters, and presentation logic for orders.
- `pub mod ui`: Leptos admin page component (`OrderAdmin`).
- `pub use ui::OrderAdmin`: Top-level re-export for admin routing.

## Minimum Contract Set

### Input DTOs/Commands
- Order list filters, pagination parameters, and status selectors.
- Order detail presentation view models carrying lines, pricing, fulfillment status, and customer notes.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Order filter normalization: trims whitespace and handles default pagination limits.
- Order status transitions strictly reflect domain lifecycle states.

### Events / Outbox Side Effects
- UI adapter package; does not dispatch events directly; mutations are sent to the backend GraphQL / native endpoints.

### Errors / Failure Codes
- Network and mutation errors are rendered via typed admin UI error notices.
