# Crate API: rustok-finance-admin

## Public surface

### Core
- `FinanceAdminTableRow`: Table row representation for operational finance and cash desks data grid.
- `filter_rows`: Pure in-memory filtering function for finance records by title or status.

### Transport
- `FinanceAdminTransport`: Decoupled transport facade for finance administration endpoints.

### UI Components
- `FinanceAdmin`: View component mounting operational finance administration workspace.
- `FinanceAdminPage`: Full-page view component for finance management.

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) administration package for `rustok-finance`.
- Data grid display via `rustok-grid` and `rustok-grid-leptos`.
- Internationalization via `rustok-ui-i18n` with package-owned translations (`en`, `ru`).

## Domain Collaborations
- Collaborates with `rustok-finance` domain services for cash desk journals and reconciliation.
- Coordinates with billing and accounting modules for monetary state consistency.

## Minimum Contract Set

### Input DTOs/Commands
- `FinanceAdminTableRow`: Input row model for data grid rendering.
- `filter_rows`: Pure query filter taking row slice and search keyword.

### Domain Invariants
- Zero UI framework dependencies inside `core`; all filtering and row mapping are pure Rust.
- Monetary balances and transaction postings enforce currency and non-negative constraints.
- Transport facade encapsulates network execution without exposing raw transport details.

### Events / Outbox Side Effects
- Emits journal entry modifications and cash desk reconciliation actions through transport facade.

### Errors / Failure Codes
- Validation failures reported cleanly in UI without throwing panics.
- Transport failures surface user feedback via error banners.

