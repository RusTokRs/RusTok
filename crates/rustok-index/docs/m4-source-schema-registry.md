# M4 source-owned schema registry

Date: 2026-07-30

Status: `source_complete_execution_pending`

This slice publishes the source-owned schema boundary required before an honest
server-side `IndexQueryPort` composition can exist. Source modules contribute generic
`IndexSchema` contracts through `ModuleRuntimeExtensions`; the selected distribution
then materializes one immutable `Arc<SchemaRegistry>` after every compiled module has
finished runtime-extension registration.

## Ownership contract

`IndexModule` seeds an `IndexSchemaSourceCatalog`. A source module publishes a schema
with `register_index_schema_source(extensions, owner_module, schema)`.

Each catalog entry retains:

- the exact `SchemaRef` and validated `IndexSchema`;
- the calculated `SchemaFingerprint`;
- the platform module slug that owns publication, replay, and drift repair.

The owner module slug is deliberately separate from `SchemaRef.module`: the former is a
platform composition identity such as `social_graph`, while the latter is the stable
Index contract namespace such as `rustok-social-graph`.

One exact schema reference may have only one source owner. Duplicate ownership is
rejected even when both schemas have the same fingerprint. Ownership is also fixed for
the entire schema identity across versions: one source may publish v1, v2, and later
contracts, but a second source cannot take over another version of the same
`(module, entity)` identity. Both rules prevent ambiguous replay and repair authority.

## Deterministic materialization

The catalog stores entries in `BTreeMap<SchemaRef, ...>` order. The distribution calls
`materialize_index_schema_registry` only after module-owned extensions and selected
runtime bridges have registered.

All schemas are passed to `SchemaRegistry::register_batch` together. This preserves:

- atomic validation;
- deterministic schema ordering;
- links targeting schemas contributed by another source module;
- exact fingerprint and field/link type validation;
- absence of partial registry state on failure.

A missing or empty catalog returns `None`; it never publishes an empty
`SharedIndexSchemaRegistry` as a completed query runtime. A non-empty catalog publishes
one cloneable wrapper around the same immutable `Arc<SchemaRegistry>` through the module
runtime extensions consumed by executable hosts. Its constructor is private, so callers
cannot bypass catalog validation with an ad hoc registry.

## First source publication

With the Social Graph `index` feature enabled, `SocialGraphModule` declares an explicit
dependency on core module `index` and publishes
`social_graph_relation_index_schema()` under owner slug `social_graph`.

The projector continues using the same owner-defined schema and existing Index-owned
persistence APIs. This boundary does not move source authority into Index and does not make
the distribution or server import Social Graph DTOs or construct its schema directly.

## Runtime handoff

The follow-up query-runtime slice is now source complete. The server invokes
`materialize_postgres_index_query_runtime` only after distribution has published the final
`SharedIndexSchemaRegistry`. That Index-owned materializer binds the immutable registry to
the host database and publishes `SharedIndexQueryRuntime` without executing SQL or claiming
tenant readiness. See `m4-query-runtime-composition.md`.

## Remaining boundary

This registry boundary still does not:

- connect the runtime to storefront/admin/search or other query consumers;
- authorize callers or construct transport-facing queries;
- persist tenant schema readiness or replace `PostgresSchemaRegistrationStore`;
- change mutation delivery, replay, or Social Graph source storage;
- add schemas for Product, Content, Flex, or other owners;
- add ordering through a `many` relation;
- execute PostgreSQL/reference capture or admission;
- authorize production partition lifecycle work.

## Owner validation

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
cargo test -p rustok-index source_schema_registry -- --nocapture
cargo test -p rustok-social-graph --features index module_publishes_its_index_schema_through_runtime_extensions -- --nocapture
cargo test -p rustok-distribution source_schema_catalog_materializes_after_all_modules_register -- --nocapture
cargo test -p rustok-distribution empty_source_catalog_does_not_publish_false_query_registry -- --nocapture
cargo check -p rustok-index --all-targets
cargo check -p rustok-social-graph --features index --all-targets
cargo check -p rustok-distribution --all-targets
node scripts/verify/verify-index-source-schema-registry.mjs
node scripts/verify/verify-index-query-runtime-composition.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo xtask module validate index
```
