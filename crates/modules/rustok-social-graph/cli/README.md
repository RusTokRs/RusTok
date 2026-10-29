# RusToK Social Graph CLI

Operational command adapters for the Social Graph module.

## Responsibilities

- Parse social-graph maintenance commands.
- Invoke Social Graph owner contracts through the shared runtime.
- Preserve tenant, actor, and idempotency evidence.

## Interactions

The crate consumes `rustok-social-graph`, `rustok-api`, and `rustok-cli-core`; it does not own graph persistence.

## Entry points

- `src/lib.rs` — command registration and dispatch.

## Documentation
- [CRATE_API.md](CRATE_API.md)

See the [Social Graph documentation](../rustok-social-graph/docs/README.md) and the [module documentation map](../../../docs/modules/_index.md).
