# rustok-tenant-admin

Leptos admin UI package for the `rustok-tenant` module.

## Responsibilities

- Exposes the tenancy overview used by `apps/admin`.
- Keeps tenant-specific runtime visibility inside the module package.
- Participates in the manifest-driven admin UI composition path through `rustok-module.toml`.
- Uses native Leptos `#[server]` functions for the bootstrap surface.

## Entry Points

- `TenantAdmin` - root admin page component for the module.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Mounted by the Leptos admin host under `/modules/tenant`.
- Reads tenant state and module enablement directly from the server runtime instead of going through GraphQL.

## Documentation

- See [platform docs](../../../../docs/index.md).
