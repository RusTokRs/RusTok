# M4 many-link `EXISTS` filtering

This slice adds deterministic PostgreSQL filtering through explicit paths that cross one
or more `many` links. It does not add many-link projection, ordering, SQL execution, or
PostgreSQL/reference-engine equivalence evidence.

## Plan contract

`PlannedJoin` and `PlannedField` now carry `traverses_many`. The flag becomes true on
the first `LinkCardinality::Many` step and remains true for every descendant join and
field. The executable-plan fingerprint domain is therefore bumped to
`rustok-index-query-plan-v3`.

The compiler recalculates the propagation rule before SQL emission. Missing joins,
inconsistent flags, invalid aliases, many-link projection, and many-link ordering fail
closed with typed errors.

## Outer rowset boundary

Only joins whose full path does not traverse `many` are emitted into the main query
`FROM` clause and compiled identity-column contract. Many-filter paths never become
ordinary outer joins.

This preserves exactly one main result row per root entity, so root ordering, keyset and
offset pagination, one-row lookahead, result decoding, and `COUNT(*)` are not corrupted
by child multiplicity.

## Correlated path traversal

Every atomic predicate on a many-traversing field compiles an independent nested
correlated `EXISTS` chain. Each link step binds its link name and exact target module,
entity, and schema version, correlates the complete source identity and source version,
and joins only a live target `index_entities` row.

Independent atomic subqueries are intentional. For example,
`And(Eq(variants.color, red), Eq(variants.size, large))` may be satisfied by two
different variants, matching the reference engine's flattened path-value semantics.

## Operator semantics

Positive operators use existential semantics over all reachable terminal values:

- `Eq`, `In`, and range operators succeed when any reachable scalar matches;
- `Contains` succeeds when any reachable list contains the requested value;
- `IsNull(path, false)` succeeds when any reachable field value is non-null;
- `IsNull(path, true)` is the negation of that non-null existence test, so an empty
  path, missing fields, and all-null values are null.

`Ne` is deliberately not compiled as `NOT EXISTS(Eq)`. It requires both:

1. at least one reachable stored field value; and
2. no reachable stored value that is tagged null, malformed, or equal to the requested
   value.

This matches the reference contract: an empty path is false, any null disqualifies the
result, and every present value must differ.

Logical `Not`, `And`, and `Or` wrap these total-boolean atomic expressions without SQL
`NULL` leakage.

## Bind and count guarantees

Tenant, schema, locale, link metadata, terminal field names, and filter values remain
ordered bind values. Source-domain names are never interpolated into SQL.

The optional exact-count statement recompiles the same correlated filters with its own
ordered bind list. It contains no many outer join, ordering, cursor predicate, limit, or
offset, so each matching root contributes exactly one count row.

## Remaining boundary

Many-link projection remains rejected with `ManyLinkProjectionPending` until an explicit
nested aggregation result shape is designed. Many-link ordering remains rejected until
an aggregate ordering policy exists.

This slice also does not:

- prepare or execute PostgreSQL statements;
- adapt SeaORM rows or bind values;
- publish `IndexQueryPort`;
- verify persisted schema/index readiness;
- authorize callers;
- claim plan/SQL snapshots or PostgreSQL/reference-engine equivalence;
- change migrations or partition lifecycle state.

The repository owner runs formatting, compilation, tests, static verifiers, and later
live equivalence evidence.
