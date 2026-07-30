# M4 controlled PostgreSQL query compiler

This M4 chain compiles the database-independent `ExecutableQueryPlan` into
controlled PostgreSQL statements plus ordered typed bind lists. The compiler does not
connect to PostgreSQL or execute SQL. Deterministic compiled-row decoding lives in a
separate application handoff; execution remains owned by `PostgresIndexQueryPort`.

## Planner contract

`SchemaRegistry::plan_query` records every referenced field exactly once in
`ExecutableQueryPlan::referenced_fields`. Each `PlannedField` carries:

- the validated `FieldPath`;
- the deterministic relation alias;
- `IndexValueType`;
- field cardinality;
- nullability;
- whether the path traverses at least one many-cardinality link.

`PlannedJoin` carries the same propagated `traverses_many` boundary. Projected fields
that traverse many are grouped into `PlannedManyProjection` values by terminal link
path. Each group preserves first-path occurrence, selected field order, and every
relation identity prefix. The plan fingerprint domain is
`rustok-index-query-plan-v4`, so older or altered nested-result plans cannot be
confused with current compiler input.

Aggregate ordering does not add plan fields. Explicit `min_*` / `max_*` directions use the
existing `PlannedOrder` and mark the derived order field nullable because an empty or all-null
relation set yields SQL `NULL`. Existing `asc` / `desc` variants retain their original enum
positions and plan representation.

## Supported query semantics

`SchemaRegistry::compile_postgres_query` validates and plans the query, decodes
and validates any continuation cursor, and then compiles:

- root projection and projection through explicit one-cardinality links;
- row-aligned nested projection through paths crossing one or more many links;
- `And`, `Or`, `Not`, `Eq`, `Ne`, `In`, range, `Contains`, and `IsNull` filters;
- filtering through paths that cross one or more many-cardinality links;
- typed PostgreSQL casts for boolean, integer, decimal, string, UUID, and
  timestamp fields;
- deterministic multi-column root/one-link ordering with explicit null placement;
- explicit `MIN` / `MAX` ordering through many links for bounded offset pages;
- lexicographic keyset continuation for ordinary non-aggregate ordering;
- bounded cursor limits;
- bounded compatibility offset pagination;
- an optional separate exact-count statement over the same scope and filters,
  without cursor, limit, offset, or projection leakage.

The main page statement selects hidden tagged JSONB order values when explicit
ordering is present. Ordinary cursor pages use those values to construct the next
`IndexCursor`. Aggregate cursor pagination remains rejected by both validator and compiler.

## Predicate and ordering contract

Scalar fields are extracted from the tagged JSONB storage value and cast to the
registered field type. Field names and scalar/list values remain bind parameters.
Strings use `COLLATE "C"`; timestamps use `timestamptz`.

Atomic predicates are compiled into total booleans. Missing linked rows, absent
fields, and tagged null values do not leak SQL `NULL` into `Not`, `And`, or `Or`
semantics:

- equality, membership, range, and containment use `COALESCE(..., FALSE)`;
- root/one-link `Ne` is true only for an existing non-null scalar that differs;
- root/one-link `IsNull` treats an absent field, missing linked row, or tagged null as
  null.

Ascending order uses `NULLS LAST`; descending order uses `NULLS FIRST`, followed
by the invariant ascending root `entity_id` tie-breaker. Plain `asc` / `desc` through a
many link remains rejected. Callers must select `min_asc`, `min_desc`, `max_asc`, or
`max_desc`.

Many-link aggregate ordering is accepted only for sortable scalar integer, string, or
timestamp fields and only with bounded offset pagination. Boolean, Decimal, UUID, list-valued,
singular-path, cursor-paginated, and unsortable aggregate orders fail closed.

Decimal ordinary filters and root/one-link ordering remain supported. Only Decimal many-link
aggregation is deferred because the hidden order column must round-trip through the exact tagged
`IndexValue` JSON representation; the current PostgreSQL numeric JSON emission is not yet an
admitted exact `rust_decimal` wire contract.

## Many-link filter boundary

Many-traversing joins are excluded from the main `FROM` clause and compiled outer
identity columns. Every atomic filter on such a field emits an independent nested
correlated `EXISTS` chain over `index_links` and live `index_entities` targets.
Complete source identity, source version, link name, and exact target schema identity
are checked at each hop.

Independent atomic subqueries preserve reference semantics when separate children
satisfy separate logical branches. Positive operators use any-match behavior.
`IsNull(path, true)` means no reachable non-null value exists.

Many-link `Ne` is intentionally stricter than `NOT EXISTS(Eq)`: it requires at least
one stored reachable value and also requires that no reachable value is tagged null or
equal to the requested value.

Because no many join enters the outer rowset, root keyset/offset pagination,
one-row lookahead, and `COUNT(*)` remain duplicate free.

## Nested many-link projection boundary

Each `PlannedManyProjection` compiles into one correlated scalar JSONB aggregate.
The subquery traverses the complete explicit relation path but never joins that path
into the outer root rowset. Empty relations return `[]`.

Every aggregate item stores aligned arrays for:

- the UUID at every relation-path prefix;
- the tagged `IndexValue` for every selected field sharing the terminal path.

Stored list fields remain one tagged list value inside their owning relation item;
they are not flattened across entities. Missing stored fields become tagged nulls and
are revalidated against source nullability by the decoder.

Aggregate rows use deterministic ordering at each link step: persisted ordinal,
target UUID, then target locale key. The decoder reconstructs
`IndexNestedRelationProjection` / `IndexNestedRelationItem`, preserving complete
ancestry and field alignment. It rejects malformed JSON, arity drift, nil identities,
duplicate identity chains, invalid tagged values, and source type/cardinality/
nullability mismatches.

## Many-link aggregate ordering boundary

Every explicit aggregate order compiles as a correlated scalar subquery rooted at the current
outer entity. The subquery walks the complete relation path and evaluates typed `MIN` or `MAX`
over the terminal scalar. It does not join the relation into the outer rowset.

PostgreSQL aggregate null behavior is the contract:

- null terminal values do not participate;
- an empty or all-null relation set produces SQL `NULL`;
- the selected `__order_N` value is SQL null or tagged `IndexValue` JSONB;
- `ORDER BY` uses the typed scalar expression, explicit null placement, and root ID tie-break.

The admitted aggregate order wire types in this slice are integer, string, and timestamp.
Decimal and UUID remain rejected until their exact hidden-order transport is independently
specified and covered by PostgreSQL/reference evidence.

Compiler validation independently rejects forged plans that omit the aggregate on a many path,
place an aggregate on a singular path, use an unsupported type, mutate nullable metadata, or
switch an aggregate plan to cursor pagination.

## Cursor boundary

Raw v1 cursor envelopes remain available only for codec round trips and the
test-only reference engine. They are not accepted by the PostgreSQL compiler.

Production continuation tokens use scoped v2 envelopes created by
`CursorCodec::encode_for_query`. The outer envelope includes a versioned
`rustok-index-cursor-query-v1` fingerprint over tenant, schema, locale, filter,
and ordered field/direction semantics. A continuation query must use
`SchemaRegistry::compile_postgres_query`, which calls
`CursorCodec::decode_scoped_for_query` and verifies:

- checksum and scoped cursor version;
- filter/order query fingerprint;
- tenant, schema, locale, and schema fingerprint;
- non-nil entity identity;
- order-value arity;
- every non-null cursor order value against the registered field type.

Changing a filter, ordered field, or order direction therefore produces
`QueryFingerprintMismatch` before keyset SQL is emitted. Projection changes alter the
v4 executable plan and compiled-column contract without invalidating a cursor whose
filter/order scope is otherwise unchanged. Aggregate directions are rejected before cursor
encoding or continuation until a separate derived-value cursor contract exists.

## Bind and SQL boundary

Tenant UUID, schema identities, locale, link metadata, field names, filter values,
cursor values, limit, and offset are bind values. Only fixed
`index_entities`/`index_links` table and column names plus compiler-owned `tN`, `lN`,
`mx_*`, `mpN_*`, and `mo_*` aliases appear in SQL.

The compiler contract, SQL emission, and page/result handoff live in separate modules:

- `application/planner.rs` owns propagated many metadata and nested projection groups;
- `application/postgres_compiler.rs` owns public SQL/bind types, scoped cursor entry
  points, compiled column metadata, and plan invariant checks;
- `application/postgres_query_sql.rs` owns controlled SQL, correlated many-link
  predicates/projections/order aggregates, and deterministic bind emission;
- `application/postgres_query_result.rs` owns one-row lookahead wrapping, strict
  compiled-column and nested-payload decoding, exact count, `has_more`, and scoped
  next cursors.

## Page and result handoff

`SchemaRegistry::compile_postgres_page_query` calls the validated compiler and changes
only the main page-limit bind from `N` to `N + 1`. The SQL string, filters, projection,
ordering, keyset predicate, offset, plan fingerprint, and optional count statement
remain unchanged.

`PostgresIndexQueryPort` maps driver values into `CompiledPostgresRow`.
`decode_postgres_query_page` then rechecks the v4 plan fingerprint and complete
`CompiledQueryColumn` contract, validates flat and nested tagged values, removes the
lookahead row, and returns `IndexQueryPage`.

## Exact count

`include_exact_count` produces `CompiledPostgresCount` as a separate controlled
statement with its own ordered bind list. The count applies tenant/schema/locale,
live-row, non-many outer joins, and all filters, but deliberately excludes projection,
keyset, ordering, limit, and offset. Many filters remain correlated subqueries, so
child multiplicity cannot inflate the count.

## Remaining fail-closed semantics

Aggregate cursor continuation remains rejected until a stable derived-value cursor identity,
strict null/value continuation semantics, and independent reference coverage are implemented.
Decimal aggregate ordering remains rejected until an exact tagged-wire contract and retained
PostgreSQL/reference scenario exist. Compiler validation recalculates propagated many metadata
and nested projection groups, and rejects missing or inconsistent join/field/result contracts
before SQL emission.

## Non-claims

This source chain does not:

- execute PostgreSQL in this implementation pass;
- add aggregate scenarios to the retained PostgreSQL/reference fixture or evidence bundle;
- authorize callers;
- weaken persisted schema readiness checks;
- support Decimal aggregate ordering;
- support aggregate cursor continuation;
- read source-module tables;
- change migrations, runtime composition, or production partition lifecycle state.

Formatting, compilation, tests, static verifiers, and PostgreSQL/reference aggregate evidence
are reserved for the owner-operated verification phase.
