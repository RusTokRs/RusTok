# M4 nested many-link projection aggregation

This slice adds deterministic typed projection through explicit relation paths that
cross one or more `many` links. It extends the controlled planner, PostgreSQL compiler,
and compiled-row decoder only. It does not execute SQL, adapt database-driver rows,
or add aggregate ordering semantics.

## Planned result shape

`ExecutableQueryPlan` now carries `many_projections`. Every
`PlannedManyProjection` groups selected fields that share the same terminal link path.
Groups are ordered by the first selected field for that path, and fields inside a group
preserve query selection order. Interleaved selections across different many paths are
therefore deterministic without requiring callers to reorder the query.

Each group records every relation-path prefix in `identity_paths`. A projected path
`variants.prices.amount`, for example, retains both `variants` and
`variants.prices` identities for every terminal row. The plan fingerprint domain is
`rustok-index-query-plan-v4` because this nested result shape is part of the executable
contract.

Before SQL emission the compiler derives the groups again from the selected fields and
requires exact equality with the plan. Missing joins, empty groups, inconsistent path
prefixes, altered field order, or incorrect many-traversal metadata fail closed.

## Root rowset boundary

Many projection paths never become joins in the outer page rowset. The main statement
still contains exactly one row per root entity and retains the existing root/one-link
identity, ordering, keyset, bounded-offset, lookahead, and exact-count semantics.

Each many projection is one correlated scalar JSONB subquery. The subquery starts at
the root identity, traverses the complete explicit link path through `index_links` and
live `index_entities`, and returns one JSON array. An empty or missing relation returns
an empty array rather than SQL `NULL`.

## Deterministic row aggregation

Every terminal relation row is encoded internally as two aligned arrays:

- `entity_ids` contains one UUID for every relation-path prefix;
- `values` contains one tagged `IndexValue` for every selected field in the group.

Missing stored fields are encoded as the tagged `IndexValue::Null` shape. Existing
list-cardinality fields remain tagged lists inside the row; they are not flattened and
do not lose their per-entity boundary.

The aggregate is ordered at every relation step by:

1. persisted link ordinal ascending;
2. target entity UUID ascending;
3. target locale key ascending.

This ordering preserves source-declared relation order while adding stable identity
tie-breakers. Duplicate target identities are already rejected by Index record
validation, and the result decoder also rejects a repeated complete identity chain.

## Public decoded result

Root and one-link projected values remain in `IndexQueryItem::fields`, with their
existing `IndexRelationIdentity` entries in `IndexQueryItem::relations`.
Many-traversing values are returned separately in
`IndexQueryItem::nested_relations`:

- `IndexNestedRelationProjection::path` identifies the terminal relation path;
- each `IndexNestedRelationItem` contains the complete ordered relation identity chain;
- the item's fields preserve their absolute `FieldPath`, source cardinality,
  nullability, type, and query selection order.

Keeping row identities and field values together avoids the false alignment produced
by independent per-field arrays. Consumers can distinguish two variants that carry
different combinations of projected attributes, and deeper many/one paths retain their
ancestry.

## Decoder boundary

`CompiledQueryColumn::ManyRelation` carries the exact planned group. The decoder
re-plans the query and requires exact fingerprint and column equality before reading a
row. It then validates:

- JSON array shape;
- identity and field arity;
- non-nil UUID identities;
- unique complete identity chains;
- tagged `IndexValue` decoding;
- source field type, cardinality, and nullability.

Malformed or partially aligned aggregate data never becomes a public query result.

## Remaining fail-closed semantics

Ordering through a many link remains rejected with `ManyLinkOrderingPending` until an
explicit aggregate ordering policy is selected. The existing correlated `EXISTS`
filtering contract is unchanged; every atomic filter remains independent and does not
reuse projection aggregates.

This slice does not:

- prepare or execute PostgreSQL statements;
- map `PostgresBindValue` into SeaORM/sqlx parameters;
- adapt SeaORM `QueryResult` values into `CompiledPostgresRow`;
- publish or compose `IndexQueryPort`;
- authorize callers;
- read source-module tables;
- claim plan/SQL snapshots or PostgreSQL/reference-engine equivalence;
- change migrations, ingestion, rebuild, or partition lifecycle state.

Formatting, compilation, tests, static verifiers, and live PostgreSQL equivalence are
reserved for the later owner-operated verification phase.
