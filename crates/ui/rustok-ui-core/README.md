# RusToK UI Core

Framework-agnostic route, input, busy-state, and presentation contracts shared by RusToK UI packages.

## Responsibilities

- Define reusable UI contracts without framework dependencies.
- Centralize route selection and common class composition.
- Prevent equivalent module surfaces from inventing local contracts.

## Interactions

Module-owned UI packages and hosts consume this crate; framework-specific adapters remain in their respective UI libraries.

## Entry points

- `src/lib.rs` — public exports.
- `src/route_selection.rs` — route-selection contracts.
- `src/ui.rs` — shared UI state contracts.

See the [module UI guide](../../../docs/UI/module-package-implementation.md).
