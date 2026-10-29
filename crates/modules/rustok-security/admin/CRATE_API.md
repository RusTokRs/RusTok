# rustok-security-admin / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA UI components for security administration and policy management.
- Integrates with `rustok-grid-leptos`, `rustok-ui-core`, `rustok-ui-transport`, and `rustok-ui-i18n`.

## Domain Collaborations
- Collaborates with `rustok-security` for security settings, API tokens, and policy definitions.
- Collaborates with `apps/admin` as a registered admin module package.

## Public surface
- `pub mod core`: Pure Rust view-models and search filters (`SecurityAdminTableRow`, `filter_rows`).
- `pub mod transport`: Transport facade (`SecurityAdminTransport`).
- `pub mod ui`: Admin components (`SecurityAdminPage`).
- `pub use core::{SecurityAdminTableRow, filter_rows}`
- `pub use transport::SecurityAdminTransport`
- `pub use ui::SecurityAdminPage`

## Minimum Contract Set

### Input DTOs/Commands
- `SecurityAdminTableRow`: Table row representation carrying id, title, status, timestamp, and edit/delete permissions.
- `filter_rows`: In-memory query filter for security entries.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Row filtering is case-insensitive and trims input queries.
- Default permissions `can_edit = true` and `can_delete = true` upon table row instantiation.

### Events / Outbox Side Effects
- Admin UI adapter package; does not dispatch events directly.

### Errors / Failure Codes
- Security transport errors are encapsulated and surfaced through Leptos UI state signals.
