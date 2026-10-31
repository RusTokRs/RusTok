# apps/dioxus-admin / CRATE_API

## Purpose

`rustok-admin-dioxus` (`apps/dioxus-admin`) is a dormant host shell runner and standalone workspace scaffold reserved for Phase E of the Fluid Frontend Architecture (FFA) Dioxus migration pilot. It provides an isolated Dioxus entry point for future desktop/mobile admin deployment experimentation without coupling to the primary workspace.

## Architecture & Responsibilities

- **Dormant Host Scaffold:** Reserved entry point (`src/main.rs`) for testing Dioxus desktop and web host shells once module-level `ui/dioxus.rs` adapters are activated.
- **Workspace Isolation:** Maintained intentionally outside the root Cargo workspace (`[workspace]` declared locally) to isolate Dioxus 0.7 toolchain specifics (`dx serve`) from the production Leptos 0.8 build pipeline (`cargo-leptos`).
- **Zero Domain Logic:** Contains strictly no business logic, domain models, or custom widgets; all domain capabilities belong in `crates/modules/*/admin` and `crates/ui/rustok-ui/dioxus`.

## Public Surface

- `main()`: Entry point binary for standalone runner invocation.

## Invariants & Guarantees

1. **Workspace Decoupling:** Must never be added to root `Cargo.toml` `[workspace.members]` without explicit architecture decision records (ADRs).
2. **Zero Inlined Domain Rules:** Never implements platform or module business logic in the host runner.
3. **No Production Exposure:** Remains dormant and uncalled in production deployments.

## Dependencies & Boundaries

- **Dioxus Ecosystem:** Depends on `dioxus` 0.7 (optional feature flags: `web`, `desktop`).
- **Downstream Adapters (Target):** Will mount `crates/ui/rustok-ui/dioxus` and module Dioxus adapters (`crates/modules/*/admin`) when Phase E is scheduled.
