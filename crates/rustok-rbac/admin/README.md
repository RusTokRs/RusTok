# rustok-rbac-admin

Leptos admin UI package for the `rustok-rbac` module.

## Responsibilities

- Exposes the RBAC runtime overview used by `apps/admin`.
- Keeps RBAC-specific visibility inside the module package.
- Participates in the manifest-driven admin UI composition path through `rustok-module.toml`.
- Uses native Leptos `#[server]` functions for the bootstrap surface.

## Entry Points

- `RbacAdmin` - root admin page component for the module.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Mounted by the Leptos admin host under `/modules/rbac`.
- Shows the live permission snapshot and module-declared permission catalog directly from the server runtime.

## Documentation

- See [platform docs](../../../../docs/index.md).
