# RusToK UI Transport

For the complete public API and transport invariant definitions, see [CRATE_API.md](CRATE_API.md).

Framework-agnostic transport result and error-evidence contracts for module-owned UI packages.

## Responsibilities

- Normalize transport success and failure evidence.
- Preserve request-path and error metadata across UI adapters.
- Avoid package-local transport result formats.

## Interactions

Leptos and Next-facing packages consume these contracts around native server functions, GraphQL, or REST adapters.

## Entry points

- `src/lib.rs` — public result, path, and error contracts.

See the [module UI guide](../../../docs/UI/module-package-implementation.md).
