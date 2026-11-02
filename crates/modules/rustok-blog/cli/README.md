# RusToK Blog CLI

Operational command adapters for the Blog module.

## Responsibilities

- Parse Blog maintenance commands through the shared CLI runtime.
- Invoke Blog post and preview token owner services.
- Emit structured command results and support `--dry-run`.

## Interactions

This crate depends on `rustok-blog`, `rustok-outbox`, `rustok-runtime`, and `rustok-cli-core`; it does not own blog policy or database state.

## Entry points

- `src/lib.rs` — command registration and dispatch.

## Documentation
- [CRATE_API.md](CRATE_API.md)

See the [Blog documentation](../docs/README.md) and the [module documentation map](../../../../docs/modules/_index.md).

