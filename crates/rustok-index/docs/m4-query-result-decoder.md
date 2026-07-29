# M4 PostgreSQL query result decoding

This boundary defines the strict handoff between the controlled M4 compiler,
`PostgresIndexQueryPort`, and the public typed page result. The decoder itself does not
execute SQL, prepare statements, own a database connection, or authorize callers.

## Page compilation contract

`SchemaRegistry::compile_postgres_page_query` first calls the validated
`compile_postgres_query` path. It wraps the compiled statement in the opaque,
non-serde `CompiledPostgresPageQuery` and changes only the validated page-limit bind
from `N` to `N + 1`.

The SQL text, plan fingerprint, scalar and many-relation metadata, filters, ordering,
cursor predicate, offset, and optional exact-count statement remain unchanged. Cursor
and bounded-offset pages use the same lookahead rule; the original offset bind is
rechecked and preserved.

## PostgreSQL row handoff

`PostgresIndexQueryPort` performs exact persisted-schema preflight, executes the page
and optional count inside one read-only repeatable-read transaction, and converts
SeaORM driver rows into `CompiledPostgresRow`. Cells are limited to compiler-owned
shapes:

- UUID or SQL null for outer relation identities;
- tagged `IndexValue` JSON or SQL null for scalar projection and hidden order columns;
- JSON arrays for `CompiledManyRelationColumn` aggregates;
- PostgreSQL bigint for the optional exact-count row.

The adapter reads only compiler-declared aliases. `CompiledPostgresRow` is not a generic
SQL row abstraction, and semantic validation remains owned by the decoder.

## Decoder validation

`SchemaRegistry::decode_postgres_query_page` re-plans the query and fails closed unless
all of the following match:

1. the executable-plan v4 fingerprint;
2. unique deterministic scalar and many-relation output aliases;
3. complete `CompiledQueryColumn` and `CompiledManyRelationColumn` metadata;
4. requested page size and maximum `N + 1` row count;
5. optional exact-count contract.

Scalar and hidden order values are deserialized into `IndexValue` and checked against
planned type, cardinality, and nullability. Missing optional one-link identities may
produce null; present relations with invalid or missing required values are rejected.

## Nested many-relation decoding

Each many aggregate item contains:

- `entity_ids`: one non-nil UUID for every planned relation identity prefix;
- `values`: one tagged `IndexValue` for every selected field grouped under the
  terminal relation path.

The decoder verifies exact identity and field arity, rejects nil identities, rejects
duplicate complete identity chains, validates every tagged value, and reconstructs:

- `IndexNestedRelationProjection` for the grouped path;
- `IndexNestedRelationItem` for each reachable relation chain;
- ordered `IndexRelationIdentity` entries for every path prefix;
- ordered `IndexProjectedValue` entries aligned with the original selection.

Many-filter-only paths add no output columns because their joins remain inside
correlated `EXISTS` predicates.

## Page output

`IndexQueryPage` contains root items with:

- root entity identity;
- projected outer one-link relation identities;
- flat scalar fields;
- deterministic nested many-relation projections;
- optional exact count;
- `has_more` from the one-row lookahead;
- optional query-scoped continuation cursor.

The lookahead row is never exposed. Cursor pages encode the last retained root and
hidden order tuple through `CursorCodec::encode_for_query`. Offset pages do not
synthesize cursors.

## Retained source evidence

`postgres_many_projection_tests` covers valid aligned nested arrays and fail-closed
identity arity, field arity, nil identity, and duplicate-chain cases.
`query_snapshot_tests` retains the exact compiled many-relation metadata beside the
v4 plan and SQL fixtures.

## Remaining boundaries

The query execution path still does not:

- define aggregate many-link ordering;
- compose into server/storefront/admin/search consumers;
- authorize callers;
- provide live PostgreSQL/reference-engine equivalence evidence;
- change migrations or production partition lifecycle state.

The repository owner runs formatting, compilation, tests, static verifiers, and later
live equivalence evidence. None were run by the implementation agent.
