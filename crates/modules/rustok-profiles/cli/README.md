# RusToK Profiles CLI

Operational command adapters for the Profiles module.

## Responsibilities

- Parse profile maintenance commands.
- Invoke Profiles owner services with authenticated runtime context.
- Return structured results without duplicating profile policy.

## Interactions

The crate composes Profiles with Auth, Customer, Tenant, and Outbox owner contracts through the shared CLI runtime.

## Entry points

- `src/lib.rs` — command registration and dispatch.

See the [Profiles documentation](../docs/README.md) and the [module documentation map](../../../../docs/modules/_index.md).
