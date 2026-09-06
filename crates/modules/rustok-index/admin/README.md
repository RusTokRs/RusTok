# rustok-index-admin

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../docs/UI/module-package-verification.md)

Leptos admin UI adapter package for the `rustok-index` module.

## Responsibilities

- Exposes the index module operator diagnostics dashboard used by `apps/admin`.
- Keeps index-specific operator visibility inside the module package (M11 Admin requirement).
- Displays registered schemas catalog (names, versions, SHA-256 fingerprints, fields, links).
- Displays storage foundation (canonical tables, PostgreSQL JSONB storage layout, partition policy).
- Displays replay and reconciliation operations status (bounded replay runner, multi-pass reconciliation, drift repair).
- Participates in the manifest-driven admin UI composition path through `rustok-module.toml`.
- Keeps the admin surface in FFA shape: Leptos-free `core.rs`, module-owned `transport/` facade, and explicit `ui/leptos.rs` render adapter.
- Uses native Leptos `#[server]` functions for the bootstrap surface; this overview is currently a documented native-only single-adapter state because no public external operator contract exists yet.

## Entry Points

- `IndexAdmin` - re-exported root admin page component for the module with tabbed diagnostics views.
- `src/core.rs` - framework-agnostic view-model and error formatting helpers (100% Leptos-free).
- `src/transport/` - native server-function bootstrap facade.
- `src/ui/leptos.rs` - Leptos render/bind adapter with responsive tabs and KPI stat cards.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Mounted by the Leptos admin host under `/modules/index`.
- Reads tenant-scoped index schemas, storage metadata, and replay status directly from the server runtime.

## Documentation

- See [platform docs](../../../docs/index.md).
