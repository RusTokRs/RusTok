# rustok-runtime

## Purpose

`rustok-runtime` owns small host runtime helpers used while moving server and module
adapters away from framework-specific runtime contexts.

## Responsibilities

- Provide the neutral import surface for host runtime access helpers.
- Keep typed shared-handle lookup errors consistent across backend adapters.
- Keep executable runtime helper code outside `rustok-api` as the API contract crate is
  reduced back to stable contracts.
- Own the single portable instance-layout contract used by installer, server,
  CLI, deployment agents, workers, and runtime adapters.
- Resolve `RUSTOK_INSTANCE_ROOT`, prepare the owned root restart-safely, and
  reject paths that escape it when executable adapters derive local subtrees.
- Provide the node-local static-role materialization primitive: it rehashes
  already pre-staged bytes, creates an immutable digest-addressed role
  directory, and records a non-authoritative restart receipt. It never pulls
  a registry, interprets a tag, selects a release, runs DDL, or starts an
  arbitrary command.

## Entry Points

- `HostRuntimeContext`
- `RuntimeComposition` for host-neutral DB, settings and typed-handle composition.
- `RuntimeComposition::from_environment` for the CLI bootstrap (`RUSTOK_DATABASE_URL` or
  `DATABASE_URL`, optional `RUSTOK_SETTINGS_JSON`, and `RUSTOK_INSTANCE_ROOT`).
- `RuntimeComposition::instance_root`, `RuntimeComposition::instance_path`, and
  `resolve_instance_root_from_environment`.
- `InstancePlacement`, `InstanceLayout`, read-only
  `inspect_instance_layout`, `bind_instance_placement`, and
  `prepare_instance_layout`. Runtime inspection never claims an unprepared
  directory; installer binding is the only ownership transition.
- `RoleMaterializationRequest`, `materialize_role`, and
  `RoleMaterializationReceipt` for the narrow deployment-agent filesystem
  boundary.
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
