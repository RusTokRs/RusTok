# rustok-workflow-admin

Leptos admin UI package for the `rustok-workflow` module.

## Responsibilities

- Exposes the workflow admin root view used by `apps/admin`.
- Keeps workflow-specific admin UI inside the module boundary instead of `apps/admin`.
- Keeps status/table/template presentation rules in framework-agnostic `src/core.rs` so future host adapters can reuse the same view-model mapping without depending on Leptos runtime.
- Participates in manifest-driven host composition through `rustok-module.toml`.

## Entry Points

- `WorkflowAdmin` — root admin page component re-exported from `src/ui/leptos.rs`.
- `src/core.rs` — framework-agnostic FFA slice for workflow row view-models, status presentation, template category styling, and template-name normalization.
- `src/transport/mod.rs` — thin module-owned transport facade that currently delegates to the GraphQL adapter and gives the next native/server-function adapter a stable insertion point.
- `src/ui/leptos.rs` — Leptos-only render adapter; crate root only wires modules and re-exports `WorkflowAdmin`.
- `rustok-module.toml [provides.admin_ui]` advertises `leptos_crate`, `route_segment`, `nav_label`, and manifest-driven nested subpages such as `templates`.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Mounted by the Leptos admin host under `/modules/workflow` and `/modules/workflow/templates` through the generic module page route contract.
- Leptos adapter uses shared auth and the module-owned transport facade, which currently preserves the existing GraphQL adapter.
- Uses shared `UiRouteContext` to branch between overview and templates without `apps/admin` knowing workflow-specific routes.
- Temporarily links workflow rows back to the legacy `/workflows/:id` detail flow until the full editor also moves behind the module-owned contract.

## Documentation

- See [platform docs](../../../docs/index.md).
