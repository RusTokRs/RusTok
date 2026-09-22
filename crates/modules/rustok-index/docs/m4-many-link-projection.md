# M4 nested many-link projection aggregation

This slice adds deterministic typed projection through explicit relation paths that
cross one or more `many` links. It extends the controlled planner, PostgreSQL compiler,
and compiled-row decoder only. It does not execute SQL, adapt database-driver rows, or
add aggregate ordering semantics.

## Planned result shape

`ExecutableQueryPlan` carries `many_projections`. Every `PlannedManyProjection` groups
selected fields sharing the same terminal link path. Groups are ordered by first query
encounter, and fields inside a group preserve selection order.

Each group records every relation-path prefix in `identity_paths`. A projected path
`variants.prices.amount`, for example, retains both `variants` and `variants.prices`
identities for every terminal row. The plan fingerprint domain is
`rustok-index-query-plan-v4` because grouped nested result metadata is part of the
executable contract.

Before SQL emission the compiler derives groups again from selected fields and requires
exact equality with the plan. Missing joins, empty groups, altered field order,
inconsistent path prefixes, or invalid many-traversal metadata fail closed.

## Root rowset boundary

Many projection paths never become joins in the outer page rowset. The main statement
still contains exactly one row per root entity and retains root/one-link identity,
ordering, keyset, bounded-offset, lookahead, and exact-count semantics.

Each many projection is a correlated JSONB aggregate subquery. It starts at the root
identity, traverses the complete explicit path through `index_links` and live
`index_entities`, and returns one JSON array. Empty or missing relations return an empty
array rather than SQL `NULL`.

## Deterministic row aggregation

Every terminal relation row is encoded internally as two aligned arrays:

- `entity_ids` contains one UUID for every relation-path prefix;
- `values` contains one tagged `IndexValue` for every selected field in the group.

Missing stored fields use the tagged `IndexValue::Null` shape. List-cardinality fields
remain tagged lists inside their terminal row and are not flattened.

Aggregation order includes, for every path step:

1. persisted link ordinal ascending;
2. target entity UUID ascending;
3. target locale key ascending.

This preserves source-declared relation order with stable identity tie-breakers. Record
validation rejects duplicate link targets, and the decoder independently rejects a
repeated complete identity chain.

## Compiler metadata

Nested aggregate columns are not variants of `CompiledQueryColumn`. They are described
separately by `CompiledManyRelationColumn`, which carries:

- the compiler-owned output alias `__many_N`;
- the complete `PlannedManyProjection` contract.

`CompiledPostgresQuery::columns` therefore remains the scalar/root/order contract, while
`CompiledPostgresQuery::many_relations` describes nested aggregates. The decoder checks
uniqueness across both alias sets and exact equality with a freshly planned query.

## Public decoded result

Root and one-link values remain in `IndexQueryItem::fields`, with one-link identities in
`IndexQueryItem::relations`. Many-traversing values are returned separately in
`IndexQueryItem::nested_relations`:

- `IndexNestedRelationProjection::path` identifies the terminal relation path;
- each `IndexNestedRelationItem` contains the complete ordered identity chain;
- item fields preserve absolute `FieldPath`, type, cardinality, nullability, and query
  selection order.

Keeping identities and values together avoids false alignment from independent
per-field arrays and preserves ancestry through deeper many/one paths.

## Decoder boundary

The decoder re-plans and requires exact v4 fingerprint, scalar metadata, and
`CompiledManyRelationColumn` metadata before reading rows. It then validates:

- JSON array shape;
- identity and field arity;
- non-nil UUID identities;
- unique complete identity chains;
- tagged `IndexValue` decoding;
- source field type, cardinality, and nullability.

Malformed or partially aligned aggregate data never becomes a public query result.

## Retained snapshots

A fixed canonical many-projection query now retains:

- readable v4 plan metadata;
- the complete exact controlled SQL statement;
- ordered binds, scalar columns, and `CompiledManyRelationColumn` metadata.

The snapshots are compared byte-for-byte by `query_snapshot_tests` and cannot update
themselves. They are source evidence only until the owner runs the suite.

## Remaining fail-closed semantics

Ordering through a many link remains rejected with `ManyLinkOrderingPending` until an
explicit aggregate ordering policy is selected. Correlated `EXISTS` filtering is
unchanged and does not reuse projection aggregates.

This source does not:

- prepare or execute PostgreSQL statements;
- map `PostgresBindValue` into SeaORM/sqlx parameters;
- adapt SeaORM `QueryResult` into `CompiledPostgresRow`;
- publish or compose `IndexQueryPort`;
- authorize callers;
- read source-module tables;
- claim PostgreSQL/reference-engine equivalence;
- change migrations, ingestion, rebuild, or partition lifecycle state.

Formatting, compilation, tests, static verifiers, and live PostgreSQL equivalence remain
owner-operated. None were run by the implementation agent.
