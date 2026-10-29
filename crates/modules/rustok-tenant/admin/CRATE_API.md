# rustok-tenant-admin / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA UI components for tenant management and module enablement inspection.
- Integrates with `rustok-ui-core`, `rustok-ui-i18n`, and `leptos-auth`.

## Domain Collaborations
- Collaborates with `rustok-tenant` and `rustok-modules` for tenancy lifecycle, configurations, and active module states.
- Collaborates with `apps/admin` as a registered admin module package.

## Public surface
- `pub use ui::leptos::TenantAdmin`: Root admin view component for tenancy operations.

## Minimum Contract Set

### Input DTOs/Commands
- Tenant configuration view models, active module summaries, and tenant status indicators.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Tenant context is authoritative from host runtime; package does not invent tenant credentials.

### Events / Outbox Side Effects
- UI adapter package; tenant mutations delegate events to `rustok-tenant`.

### Errors / Failure Codes
- Tenancy and auth errors are captured and presented through Leptos error notice signals.
