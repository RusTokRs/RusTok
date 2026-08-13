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
- The normalized `RUSTOK_INSTANCE_ROOT` plus bounded instance-relative path
  resolution for executable storage, source, work, cache, and release adapters.
- `InstancePlacement`, read-only `inspect_instance_layout`, and
  `InstanceLayout` as the one physical-tree vocabulary for installer, server,
  CLI, deployment agents, workers, and runtime adapters;
  `bind_instance_placement` and `prepare_instance_layout` provide restart-safe
  ownership markers and managed directory creation.
- Typed digest paths convert protocol `sha256:<hex>` identities to portable
  `<hex>` directory segments, validate role/target segments, and cannot escape
  the selected root.
- `materialize_role` accepts only an owner-issued, digest-bound role request;
  it verifies an already pre-staged cache file, atomically materializes it under
  `releases/platform/sha256/<bundle>/<role>`, and writes a restart-safe local
  receipt. PostgreSQL rollout state remains authoritative; this helper has no
  registry, process, migration, release-selection, or traffic-switching
  authority.
- `db_clone` for explicit DB handle cloning from host runtime context.
- `require_shared` and `RuntimeHandleError` for typed shared-handle lookup.

Use this crate when the same runtime lookup pattern appears in multiple backend adapters.
Do not copy shared-handle lookup code into each module or push executable helpers back into
`rustok-api`.

Related guide: [Backend Module Implementation](../../../docs/backend/module-backend-implementation.md).
