# M4 many-link `EXISTS` filtering

This slice provides deterministic PostgreSQL filtering through explicit paths that
cross one or more `many` links. It does not execute SQL or claim
PostgreSQL/reference-engine equivalence evidence.

## Plan contract

`PlannedJoin` and `PlannedField` carry `traverses_many`. The flag becomes true on the
first `LinkCardinality::Many` step and remains true for every descendant join and
field. The executable-plan fingerprint is now `rustok-index-query-plan-v4` because the
same plan also groups selected many fields into `PlannedManyProjection` contracts.

The compiler recalculates traversal propagation and grouped projection metadata before
SQL emission. Missing joins, inconsistent flags, invalid aliases, corrupted many
projection groups, and many-link ordering fail closed with typed errors.

## Outer rowset boundary

Only joins whose full path does not traverse `many` enter the main query `FROM` clause
and outer identity-column contract. Many-filter paths never become ordinary outer
joins. Many projections use separate correlated JSONB aggregates.

This preserves exactly one main row per root entity, so root ordering, keyset and
offset pagination, one-row lookahead, result decoding, and `COUNT(*)` are not
corrupted by child multiplicity.

## Correlated path traversal

Every atomic predicate on a many-traversing field compiles an independent nested
correlated `EXISTS` chain. Each step binds the link name and exact target module,
entity, and schema version, correlates complete source identity and source version,
and joins only a live target `index_entities` row.

Independent atomic subqueries are intentional. For example,
`And(Eq(variants.color, red), Eq(variants.size, large))` may be satisfied by different
variants, matching the reference engine's flattened path-value semantics.

## Operator semantics

Positive operators use existential semantics across reachable terminal values:

- `Eq`, `In`, and range operators succeed when any reachable scalar matches;
- `Contains` succeeds when any reachable list contains the requested value;
- `IsNull(path, false)` succeeds when any reachable value is non-null;
- `IsNull(path, true)` succeeds when no reachable non-null value exists.

`Ne` is deliberately not compiled as `NOT EXISTS(Eq)`. It requires at least one
reachable stored value and no reachable tagged null or equal value. An empty path is
false, any null disqualifies the root, and every present value must differ.

Logical `Not`, `And`, and `Or` wrap total-boolean atomic expressions without SQL
`NULL` leakage.

## Bind and count guarantees

Tenant, schema, locale, link metadata, terminal field names, and filter values remain
ordered bind values. Source-domain names are never interpolated into SQL.

The optional exact-count statement recompiles the same correlated filters with its own
bind list and excludes projection aggregation, ordering, cursor, limit, and offset.
Each matching root therefore contributes exactly one count row.

## Projection interaction

Many-link projection is supported independently through grouped correlated JSONB
aggregates. It does not alter filter semantics or move many joins into the outer
rowset. Complete relation identity chains and aligned tagged values are decoded into
`IndexNestedRelationProjection`.

Many-link ordering remains rejected until an explicit aggregate ordering policy exists.

## Retained source evidence

Compiler tests cover correlated filtering, grouped many projection, duplicate-free
exact count, and tampered traversal/projection metadata. The canonical v4 plan/SQL
snapshots lock the separation between the outer root rowset, correlated aggregate, and
ordered bind envelope.

## Remaining boundary

This source does not:

- prepare or execute PostgreSQL statements;
- adapt SeaORM rows or bind values;
- publish `IndexQueryPort`;
- verify persisted schema/index readiness;
- authorize callers;
- claim PostgreSQL/reference-engine equivalence;
- change migrations or partition lifecycle state.

The repository owner runs formatting, compilation, tests, static verifiers, and later
live equivalence evidence. None were run by the implementation agent.
