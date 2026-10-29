# rustok-party-admin / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA UI components for Counterparty & Legal Entities administration.
- Integrates with `rustok-grid-leptos` and `rustok-ui-core` for data grid presentation and styling.
- Integrates with `rustok-ui-i18n` for multi-locale support.

## Domain Collaborations
- Collaborates with `rustok-party` domain capabilities.
- Collaborates with `apps/admin` as an admin module package.

## Public surface
- `pub mod core`: Pure Rust view-models and search filtering (`PartyAdminTableRow`, `filter_rows`).
- `pub mod transport`: Transport facade connecting UI to backend capabilities (`PartyAdminTransport`).
- `pub mod ui`: Leptos admin page components (`PartyAdmin`, `PartyAdminPage`).
- `pub use core::{filter_rows, PartyAdminTableRow}`
- `pub use transport::PartyAdminTransport`
- `pub use ui::{PartyAdmin, PartyAdminPage}`

## Minimum Contract Set

### Input DTOs/Commands
- `PartyAdminTableRow`: Table row representation carrying id, title, status, creation timestamp, and capability flags (`can_edit`, `can_delete`).
- `filter_rows`: Pure in-memory filtering by title or status query.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework dependencies.
- Row filtering is case-insensitive and trims input queries.
- Default permissions `can_edit = true` and `can_delete = true` upon table row instantiation.

### Events / Outbox Side Effects
- UI adapter crate; does not directly dispatch domain events.

### Errors / Failure Codes
- Transport failures are handled by the transport facade and exposed via Leptos signals.

