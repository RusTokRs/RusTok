# rustok-rbac-cli / CRATE_API

## Platform Capability Bindings
- Implements `CommandProvider` from `rustok-cli-core` for RBAC administrative CLI commands.
- Integrates with `rustok-runtime` and `sea-orm`.

## Domain Collaborations
- Collaborates with `rustok-rbac` for consistency reports, permission validation, and system role repair.
- Dispatched by platform operator tooling and CLI runners.

## Public surface
- `pub struct RbacCommandProvider`: Command provider implementing `CommandProvider` trait for `rbac` namespace.

## Minimum Contract Set

### Input DTOs/Commands
- `rbac consistency-report`: Command inspecting structural and semantic RBAC persistence integrity.
- `rbac repair-system-roles`: Command planning or applying canonical built-in role repairs.

### Domain Invariants
- Operates within explicit tenant context.
- System role repairs run within transactional boundaries.
- Preserves structured error outcomes and CLI evidence.

### Events / Outbox Side Effects
- CLI execution emits permission invalidation generation updates when repairs are committed.

### Errors / Failure Codes
- Command errors map to typed `CliCoreError` variants.
