# rustok-tasks-admin / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA UI components for team tasks, issues, and workflow tracking.
- Integrates with `rustok-grid-leptos`, `rustok-ui-core`, `rustok-ui-transport`, and `rustok-ui-i18n`.

## Domain Collaborations
- Collaborates with `rustok-tasks` backend module for task definitions, states, and assignees.
- Collaborates with `apps/admin` as a registered admin module package.

## Public surface
- `pub mod core`: Pure Rust view-models and in-memory search filters (`TasksAdminTableRow`, `filter_rows`).
- `pub mod transport`: Transport facade (`TasksAdminTransport`).
- `pub mod ui`: Admin components (`TasksAdmin`, `TasksAdminPage`).
- `pub use core::{TasksAdminTableRow, filter_rows}`
- `pub use transport::TasksAdminTransport`
- `pub use ui::{TasksAdmin, TasksAdminPage}`

## Minimum Contract Set

### Input DTOs/Commands
- `TasksAdminTableRow`: Task row representation carrying id, title, status, creation timestamp, and edit/delete permissions.
- `filter_rows`: In-memory query filter for task rows.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Row filtering is case-insensitive and trims input queries.
- Default permissions `can_edit = true` and `can_delete = true` upon table row instantiation.

### Events / Outbox Side Effects
- Admin UI adapter package; does not dispatch events directly.

### Errors / Failure Codes
- Transport and query errors are encapsulated and surfaced through Leptos UI state signals.
