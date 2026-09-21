# RusToK Admin (Dioxus Host Shell Scaffolding)

This directory contains the experimental standalone host shell scaffolding for **Dioxus**, implementing the target architecture outlined in [`docs/research/dioxus-ffa-ui-migration-plan.md`](../../docs/research/dioxus-ffa-ui-migration-plan.md).

## Purpose & Architecture

RusToK's **FFA (Fluid Frontend Architecture)** enforces a three-layer boundary:

```text
module/src/core.rs          <-- Framework-agnostic view-models, validation, status policies
module/src/transport/       <-- Transport facades (Native Server Functions + GraphQL)
module/src/ui/leptos.rs     <-- Leptos render adapter (Current production host)
module/src/ui/dioxus.rs     <-- Dioxus render adapter (Target swap)
```

This application (`apps/dioxus-admin`) acts as a prototype host shell that demonstrates:
1. **Direct consumption of `rustok-ui-core`**: Reusing `UiRouteContext`, `UiPaginationState`, `UiBadgeTone`, and `status_badge_class` without any Leptos runtime coupling.
2. **Framework-swappable views**: Rendering domain data without rewriting business rules, state machines, or view-models.
3. **Dual-path transport blindness**: Interacting with module transport facades via GraphQL or native adapters.

## Workspace Isolation Policy

Per project governance:
- `apps/dioxus-admin` is **intentionally NOT added** to the root `Cargo.toml` `[workspace.members]`.
- This isolation ensures that external framework dependencies or experimental features do not affect the stability, CI gates, or build times of the primary Leptos platform (`apps/admin`, `apps/server`, `apps/storefront`).

## Structure

- `src/main.rs` — Application entry point.
- `src/app.rs` — Main layout, navigation sidebar, header, and route host.
- `src/adapters/` — Pilot Dioxus component adapters demonstrating FFA reuse.
- `src/shell.rs` — Host shell configuration and context propagation.

## Running

When Dioxus CLI (`dx`) is installed:

```bash
# In this directory:
dx serve
```
