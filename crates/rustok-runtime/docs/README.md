# `rustok-runtime` Documentation

`rustok-runtime` is a backend foundation crate for host runtime composition helpers.

The crate is intentionally small. Its first role is to stop new backend adapters from
copying typed shared-handle lookup and DB access patterns while the host runtime context is
being removed.

Boundary rules:

- The crate enables only `rustok-api/runtime`; HTTP and GraphQL frameworks remain behind
  `rustok-api/server` and are not part of this dependency graph.
- Runtime contracts currently sourced from `rustok-api` may move here only when they are
  executable runtime helpers rather than stable API contracts.
- Domain services do not move here.
- HTTP response mapping belongs in `rustok-web`.
- CLI command contracts belong in `rustok-cli-core`.
- FBA provider/consumer metadata belongs in `rustok-fba`.

Current entry points:

- `HostRuntimeContext` re-export for backend adapters that need the neutral host contract.
- `RuntimeComposition` for optional DB/host handles plus a host-neutral settings snapshot.
- `RuntimeComposition::from_environment` for the external CLI bootstrap without a server
  dependency.
- `db_clone` for explicit DB handle cloning from host runtime context.
- `require_shared` and `RuntimeHandleError` for typed shared-handle lookup.

Use this crate when the same runtime lookup pattern appears in multiple backend adapters.
Do not copy shared-handle lookup code into each module or push executable helpers back into
`rustok-api`.

Related guide: [Backend Module Implementation](../../../docs/backend/module-backend-implementation.md).
