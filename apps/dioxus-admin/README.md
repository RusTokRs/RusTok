# RusToK Admin (Dioxus Host Shell Runner)

> [!CAUTION]
> **STATUS: CONCEPT / DORMANT RUNNER (FFA Phase E)**
> - **DO NOT USE IN PRODUCTION.**
> - **DO NOT USE IN CI / TEST SWEEPS:** Do NOT run test sweeps or automated refactors here.
> - **DO NOT ADD TO ROOT `[workspace.members]`:** Must remain isolated from the primary workspace.
> - **DO NOT IMPLEMENT DOMAIN UI OR FAKE SHELLS HERE:** All domain logic, view-models, and UI components belong strictly inside `crates/modules/*/admin` and `crates/ui/*`.
> 
> This host runner will be activated **ONLY** after domain modules have implemented their respective `ui/dioxus.rs` render adapters in Phase E of the FFA plan.

---

## Architectural Context & Purpose

This directory provides the reserved host runner and dedicated `Cargo.toml` needed by the Dioxus toolchain (`dx serve`), implementing the target architecture outlined in [`docs/research/dioxus-ffa-ui-migration-plan.md`](../../docs/research/dioxus-ffa-ui-migration-plan.md).

Because Cargo requires one `Cargo.toml` per package directory, and because the Dioxus toolchain (`dx serve`, `Dioxus.toml`, binary hot-patching) conflicts with the Leptos build pipeline (`cargo-leptos`, `[package.metadata.leptos]`), the Dioxus entry point lives in this dedicated host directory while sharing 100% of the platform's domain logic.

RusToK's **FFA (Fluid Frontend Architecture)** enforces a strict three-layer boundary:

```text
crates/modules/rustok-*/admin
      │
      ├── core.rs                 <-- Framework-agnostic view-models, validation, status policies (100% shared)
      ├── transport/              <-- Transport facades (Native Server Functions + GraphQL, 100% shared)
      ├── ui/leptos.rs            <-- Leptos render adapter (Current production host: apps/admin)
      └── ui/dioxus.rs            <-- Dioxus render adapter (Phase E target)
```

## Workspace Isolation Policy

Per project governance:
- `apps/dioxus-admin` is **intentionally NOT added** to the root `Cargo.toml` `[workspace.members]`.
- This isolation ensures that external framework dependencies or experimental features do not affect the stability, CI gates, or build times of the primary Leptos platform (`apps/admin`, `apps/server`, `apps/storefront`).

## Structure

- `Cargo.toml` — Dedicated package manifest isolating Dioxus dependencies from root workspace.
- `src/main.rs` — Minimal host runner entry point.

## Activation Plan

1. Implement `ui/dioxus.rs` render adapters in domain modules (`crates/modules/*`).
2. Verify adapter parity against `crates/ui/rustok-ui/dioxus`.
3. Wire the Dioxus shell into `apps/dioxus-admin/src/main.rs` and verify with `dx serve`.
