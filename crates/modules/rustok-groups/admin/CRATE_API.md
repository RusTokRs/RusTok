# Crate API: rustok-groups-admin

## Public surface

### UI Components
- `GroupsAdmin`: Root Leptos administration component for social groups, memberships, and application review.

### Public modules
- `core`: Framework-neutral UUID/locale validation, command preparation, and transport profiles.
- `application_core`: Policy locale-catalog, review, reopen, and bulk action preparation.
- `model`: Group directory, governance, and invitation models.
- `application_model`: Membership application review, snapshot, and policy revision models.
- `transport`: Unified transport facade executing native server functions and GraphQL operations.

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) administration package for `rustok-groups`.
- Dual-transport support: native Leptos server functions in SSR/hydrate, GraphQL queries in CSR/headless.
- Internationalization via `rustok-ui-i18n` with package-owned translations (`en`, `ru`).

## Domain Collaborations
- Collaborates with `rustok-groups` backend services for group hierarchy, memberships, and application lifecycles.
- Coordinates with user and authentication services for member identities.

## Minimum Contract Set

### Input DTOs/Commands
- `GroupsAdmin`: Entrypoint workspace component mounting groups administration.
- Group creation, governance update, invitation issuance, and application review commands.

### Domain Invariants
- Zero UI framework dependencies inside `core` or `application_core`.
- Group operations preserve tenant and channel isolation boundaries.
- Policy updates enforce atomic compare-and-swap (CAS) stale protection.

### Events / Outbox Side Effects
- Emits group lifecycle mutations, invitation issuances, and application approval/rejection events.

### Errors / Failure Codes
- Validation failures reported cleanly in UI without panics.
- Transport failures display accessible error alert feedback.

