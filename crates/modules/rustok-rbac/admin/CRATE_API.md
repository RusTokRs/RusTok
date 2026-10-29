# rustok-rbac-admin / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA UI components for RBAC runtime overview, roles, and permissions inspection.
- Integrates with `rustok-grid-leptos`, `rustok-ui-core`, `rustok-ui-i18n`, and `leptos-auth`.

## Domain Collaborations
- Collaborates with `rustok-rbac` for system roles, permissions catalog, and policy matrices.
- Collaborates with `apps/admin` as a registered admin module package.

## Public surface
- `pub use ui::leptos::RbacAdmin`: Root admin view component for RBAC operations.

## Minimum Contract Set

### Input DTOs/Commands
- Role catalog view models, permission definitions, and module-declared capability bindings.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Native server functions consume host-provided `HostRuntimeContext`.
- Role catalog and permission metadata are derived strictly from runtime registry without shadow storage.

### Events / Outbox Side Effects
- Read-only admin inspection package; does not emit domain events or mutate database rows directly.

### Errors / Failure Codes
- Bootstrap and server execution errors are surfaced through typed UI error signals.
