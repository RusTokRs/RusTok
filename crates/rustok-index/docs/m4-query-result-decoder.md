# M4 PostgreSQL query result decoding

This slice closes the database-independent handoff between the controlled M4
PostgreSQL compiler and a later execution adapter. It does not execute SQL,
prepare statements, own a database connection, or expose `IndexQueryPort`.

## Page compilation contract

`SchemaRegistry::compile_postgres_page_query` first calls the existing validated
`compile_postgres_query` path. It then wraps the compiled statement in
`CompiledPostgresPageQuery` and changes only the validated page-limit bind from
`N` to `N + 1`.

The SQL text, plan fingerprint, column metadata, scope predicates, filters,
ordering, cursor predicates, and exact-count statement remain unchanged. The
one-row lookahead is an execution detail used to determine `has_more`; it is not
part of the public query semantics.

Cursor and bounded-offset pages use the same lookahead rule. The original
offset bind is rechecked and preserved.

## Adapter row handoff

A later SeaORM/PostgreSQL adapter converts returned driver rows into
`CompiledPostgresRow`. Cells are intentionally limited to the shapes emitted by
the compiler:

- UUID or SQL null for relation identity columns;
- tagged `IndexValue` JSON or SQL null for projection and hidden order columns;
- a PostgreSQL bigint for the optional exact-count row.

This DTO is not a generic SQL row abstraction. It is a narrow handoff for the
controlled Index compiler output.

## Decoder validation

`SchemaRegistry::decode_postgres_query_page` re-plans the supplied query and
fails closed unless all of the following match:

1. the compiled plan fingerprint;
2. the complete compiled column contract and deterministic output aliases;
3. the requested page size;
4. the optional exact-count contract;
5. the maximum `requested + 1` result-row count.

Every tagged projection/order value is deserialized back into `IndexValue` and
validated against the planned type, cardinality, and nullability. A non-nullable
linked field may decode as null only when that explicit one-cardinality relation
identity is absent. A present relation with a missing non-nullable field is a
typed corruption error.

## Page output

The decoder returns `IndexQueryPage` with:

- root entity identities;
- deterministic explicit-link relation identities;
- projection values in query selection order;
- optional exact count;
- `has_more` derived only from the one-row lookahead;
- an optional query-scoped continuation cursor.

The lookahead row is never exposed. When a cursor page has an extra row, the
last retained item and its hidden order values produce an `IndexCursor`, which
is encoded through `CursorCodec::encode_for_query`. The resulting token remains
bound to tenant, schema, locale, filter, ordered fields, and directions.
Offset pages report `has_more` but do not synthesize a cursor.

## Fail-closed boundaries

Many-link semantics remain fail-closed in the compiler with
`ManyLinkSemanticsPending`. This decoder handles only the already-supported root
and explicit one-cardinality-link subset. It does not attempt to deduplicate
roots or aggregate many-link projections after SQL execution.

This slice also does not:

- execute SQL or convert bind DTOs into driver parameters;
- decode directly from SeaORM `QueryResult`;
- verify persisted schema/index readiness;
- authorize callers;
- provide PostgreSQL/reference-engine equivalence evidence;
- change migrations or production partition lifecycle state.

The repository owner runs formatting, compilation, tests, and static verifiers.
