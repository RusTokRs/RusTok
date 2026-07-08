# rustok-runtime

## Purpose

`rustok-runtime` owns small host runtime helpers used while moving server and module
adapters away from framework-specific runtime contexts.

## Responsibilities

- Provide the neutral import surface for host runtime access helpers.
- Keep typed shared-handle lookup errors consistent across backend adapters.
- Keep executable runtime helper code outside `rustok-api` as the API contract crate is
  reduced back to stable contracts.

## Entry Points

- `HostRuntimeContext`
- `db_clone`
- `require_shared`
- `RuntimeHandleError`

## Interactions

- Depends on `rustok-api` for the current `HostRuntimeContext` contract.
- Is consumed by server/module adapters as Loco runtime lookups are replaced.
- Does not own HTTP routing, CLI, FBA provider metadata, domain services, or UI transport.

See [docs](docs/README.md).

