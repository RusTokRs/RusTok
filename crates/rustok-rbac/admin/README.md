# rustok-rbac-admin

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../docs/UI/module-package-verification.md)

Leptos admin UI adapter package for the `rustok-rbac` module.

## Responsibilities

- Exposes the RBAC runtime overview used by `apps/admin`.
- Keeps RBAC-specific visibility inside the module package.
- Participates in the manifest-driven admin UI composition path through `rustok-module.toml`.
- Keeps the admin surface in FFA shape: Leptos-free `core.rs`, module-owned `transport/` facade, and explicit `ui/leptos.rs` render adapter.
- Uses native Leptos `#[server]` functions for the bootstrap surface; this overview is currently a documented native-only single-adapter state because no GraphQL/REST operator contract exists yet.

## Entry Points

- `RbacAdmin` - re-exported root admin page component for the module.
- `src/core.rs` - framework-agnostic view-model and error formatting helpers.
- `src/transport/` - native server-function bootstrap facade.
- `src/ui/leptos.rs` - Leptos render/bind adapter.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Mounted by the Leptos admin host under `/modules/rbac`.
- Shows the live permission snapshot and module-declared permission catalog directly from the server runtime.

## Documentation

- See [platform docs](../../../docs/index.md).
