# RusToK Blog CLI

Operational command adapters for the Blog module.

## Responsibilities

- Parse Blog maintenance commands through the shared CLI runtime.
- Invoke Blog post and preview token owner services.
- Emit structured command results and support `--dry-run`.

## Interactions

This crate depends on `rustok-blog`, `rustok-outbox`, `rustok-runtime`, and `rustok-cli-core`; it does not own blog policy or database state.

## Source layout

- [`src/lib.rs`](src/lib.rs) exposes the provider factory and public command options.
- [`src/provider.rs`](src/provider.rs) declares and dispatches the `blog` commands.
- [`src/commands/`](src/commands/) contains one adapter per operation and the
  shared option parsing for tenant IDs and batch limits.
- [`src/runtime.rs`](src/runtime.rs) resolves the database and event bus for the
  owner service calls.
- [`src/tests/`](src/tests/) covers the public contract, option parsing and
  dispatch through the provider factory.

`cargo xtask module lint blog` includes the shared `--cli` layout check. Run
`cargo test -p rustok-blog-cli --lib` to verify the adapter contract.

## Documentation
- [CRATE_API.md](CRATE_API.md)

See the [Blog documentation](../docs/README.md) and the [module documentation map](../../../../docs/modules/_index.md).

