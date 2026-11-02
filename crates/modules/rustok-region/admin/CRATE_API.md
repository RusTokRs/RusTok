# rustok-region-admin / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA UI components for the region administration dashboard.
- Integrates with `rustok-ui-core`, `rustok-ui-routing`, and `rustok-ui-i18n`.

## Domain Collaborations
- Collaborates with `rustok-region::RegionService` via native server functions.
- Collaborates with `apps/admin` as a registered admin module package.

## Public surface
- `pub use ui::RegionAdmin`: Root admin view component for region operations.

## Minimum Contract Set

### Input DTOs/Commands
- Region creation and update forms carrying name, currency code, countries list, and optional tax provider ID.
- Region list and detail view models.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Country codes and currency codes are normalized and validated.
- Tax provider selection uses typed identifier instead of untyped metadata blobs.

### Events / Outbox Side Effects
- UI adapter package; region modifications delegate domain events to `rustok-region`.

### Errors / Failure Codes
- Validation and server execution errors are surfaced through typed admin UI notices.
