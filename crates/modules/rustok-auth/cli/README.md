# RusToK Auth CLI

For full public interface and operational contract definitions, see [CRATE_API.md](CRATE_API.md).

Operational command adapters for the Auth module.

## Responsibilities

- Parse Auth CLI commands through the shared CLI runtime.
- Invoke Auth owner services without duplicating domain policy.
- Return structured command output and errors.

## Interactions

This crate depends on `rustok-auth`, `rustok-cli-core`, and the shared runtime. It does not own Auth persistence or policy.

## Entry points

- `src/lib.rs` — command registration and dispatch.

See the [Auth module documentation](../docs/README.md) and the [module documentation map](../../../../docs/modules/_index.md).
