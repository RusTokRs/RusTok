# RusToK Media CLI

Operational command adapters for the Media module.

## Responsibilities

- Parse Media maintenance commands through the shared CLI runtime.
- Invoke Media and storage owner contracts.
- Emit structured command results.

## Interactions

This crate depends on `rustok-media`, `rustok-storage`, and `rustok-cli-core`; it does not own media policy or storage state.

## Entry points

- `src/lib.rs` — command registration and dispatch.

See the [Media documentation](../docs/README.md) and the [module documentation map](../../../../docs/modules/_index.md).
