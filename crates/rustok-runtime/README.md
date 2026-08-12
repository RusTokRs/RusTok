# rustok-runtime

## Purpose

`rustok-runtime` owns small host runtime helpers used while moving server and module
adapters away from framework-specific runtime contexts.

## Responsibilities

- Provide the neutral import surface for host runtime access helpers.
- Keep typed shared-handle lookup errors consistent across backend adapters.
- Keep executable runtime helper code outside `rustok-api` as the API contract crate is
  reduced back to stable contracts.
- Resolve the single portable `RUSTOK_INSTANCE_ROOT` and reject paths that
  escape it when executable adapters derive local subtrees.

## Entry Points

- `HostRuntimeContext`
- `RuntimeComposition` for host-neutral DB, settings and typed-handle composition.
- `RuntimeComposition::from_environment` for the CLI bootstrap (`RUSTOK_DATABASE_URL` or
  `DATABASE_URL`, optional `RUSTOK_SETTINGS_JSON`, and `RUSTOK_INSTANCE_ROOT`).
- `RuntimeComposition::instance_root`, `RuntimeComposition::instance_path`, and
  `resolve_instance_root_from_environment`.
- `db_clone`
- `require_shared`
- `RuntimeHandleError`

## Interactions

- Depends on `rustok-api` for the current `HostRuntimeContext` contract and keeps settings as a
  host-neutral JSON snapshot rather than depending on server configuration types.
- Enables only the neutral `rustok-api/runtime` feature; it does not pull Axum or
  Async-GraphQL into module owners that consume runtime helpers.
- Is consumed by server/module adapters for typed runtime lookups.
- Does not own HTTP routing, CLI, FBA provider metadata, domain services, or UI transport.

See [docs](docs/README.md).
