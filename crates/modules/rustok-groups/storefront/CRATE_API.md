# Crate API: rustok-groups-storefront

## Public surface

### UI Components
- `GroupsView`: Root storefront component for group directory browsing, application submission, and invitation processing.

### Public modules
- `core`: Framework-neutral group discovery, route parsing, and invitation validation.
- `application_core`: CAS membership application preparation, answer validation, and cancellation models.
- `model`: Public group summaries, directory listings, and invitation details.
- `application_model`: Membership application forms, answers, and candidate status DTOs.
- `transport`: Unified storefront transport facade for native server functions and GraphQL operations.

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) storefront package for `rustok-groups`.
- Dual-transport support: native Leptos server functions in SSR/hydrate, GraphQL queries in CSR/headless.
- Internationalization via `rustok-ui-i18n` with package-owned translations (`en`, `ru`).

## Domain Collaborations
- Collaborates with `rustok-groups` domain services for public directory feeds and application submissions.
- Coordinates with user session and auth hooks for candidate verification.

## Minimum Contract Set

### Input DTOs/Commands
- `GroupsView`: Entrypoint storefront view mounting group discovery and application flows.
- Membership application submission and candidate cancellation commands.
- Invitation acceptance commands.

### Domain Invariants
- Secret groups are never disclosed in public directory feeds.
- Membership application submissions enforce CAS checks against the loaded policy revision.
- Zero business logic inside UI view templates; validation and mapping live in `core` and `application_core`.

### Events / Outbox Side Effects
- Emits membership application submissions and cancellation events.

### Errors / Failure Codes
- Stale policy errors return distinct recovery states prompting form reload.
- Transport failures surface accessible error notices without crashing views.

