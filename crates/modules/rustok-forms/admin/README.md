# rustok-forms-admin

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../../docs/UI/module-package-verification.md) |
> [Public API Contract](CRATE_API.md)

## Purpose

`rustok-forms-admin` provides the Leptos admin root view and lead triage inbox for the `rustok-forms` module.

## Responsibilities

- Export the module-owned `FormsAdmin` root component for `apps/admin`.
- Maintain strict FFA separation: framework-agnostic models, submission summarizers, state machine transition validators, and localized label resolvers in `src/core.rs`.
- Present incoming form submissions chronologically with honeypot spam status and interactive triage transitions.
- Expose GraphQL mutations for updating submission triage state (`new`, `read`, `handled`, `spam`).

## Public entry point

- `FormsAdmin`
