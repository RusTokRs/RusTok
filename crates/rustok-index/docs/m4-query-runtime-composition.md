# M4 Index query runtime composition

Date: 2026-07-30

Status: `source_complete_execution_pending`

This slice composes the canonical Index query capability after source-owned schemas have
been collected and materialized. The runtime is host-owned, transport neutral to consumers,
and backed by `PostgresIndexQueryPort` without exposing its database connection or mutable
registry internals.

## Runtime contract

`SharedIndexQueryRuntime` is a cloneable wrapper around one `Arc<dyn IndexQueryPort>`.
Its production constructor is crate-owned. Consumers can clone the neutral capability or
invoke the `IndexQueryPort` contract, but cannot substitute arbitrary SQL, result metadata,
or an ad hoc registry while claiming the canonical runtime.

`materialize_postgres_index_query_runtime(extensions, db)` is the single production
materializer. It:

- refuses a second `SharedIndexQueryRuntime` in the same extension set;
- returns `None` when no `SharedIndexSchemaRegistry` exists;
- reuses the exact immutable `Arc<SchemaRegistry>` produced from source publications;
- constructs `PostgresIndexQueryPort` with the host-owned `DatabaseConnection`;
- inserts the resulting neutral runtime into `ModuleRuntimeExtensions`;
- performs no SQL, migration, persisted-schema read, or tenant readiness check.

Backend support and exact tenant-scoped persisted schema readiness remain execution-time
responsibilities of `PostgresIndexQueryPort`. Runtime presence therefore means the adapter is
composed, not that a specific tenant query will succeed.

## Server composition

The public server `module_event_dispatcher` facade delegates all existing provider setup to
the retained base implementation. It then invokes the Index-owned materializer as the final
host-provider step. This ordering guarantees that:

1. every compiled module has published its schema contribution;
2. `rustok-distribution` has atomically materialized the complete source registry;
3. existing server-owned providers remain unchanged;
4. the query runtime is built from the final registry and the host database;
5. the resulting extension set transfers unchanged into `HostRuntimeContext`.

The server does not call `PostgresIndexQueryPort::new` directly and does not import any
source schema builder or owner DTO.

## Boundary

This slice does not:

- authorize callers or tenant scopes;
- add GraphQL, storefront, admin, search, or native-server query endpoints;
- cut any consumer over to Index;
- publish Product, Content, Flex, or other source schemas;
- persist tenant schema readiness;
- execute a query during startup;
- add ordering through a `many` relation;
- execute PostgreSQL/reference capture or admission;
- authorize production partition lifecycle work.

The next consumer slice must resolve `SharedIndexQueryRuntime`, apply owner/transport
authorization before constructing a typed `IndexQuery`, and preserve all query-port errors
without exposing database details.

## Owner validation

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
cargo test -p rustok-index query_runtime -- --nocapture
cargo test -p rustok-server host_materializes_index_query_runtime_after_source_registry -- --nocapture
cargo check -p rustok-index --all-targets
cargo check -p rustok-server --all-targets
node scripts/verify/verify-index-query-runtime-composition.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo xtask module validate index
```
