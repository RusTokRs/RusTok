# M4 controlled PostgreSQL query compiler

This M4 chain compiles the database-independent `ExecutableQueryPlan` into
controlled PostgreSQL statements plus ordered typed bind lists. The compiler does not
connect to PostgreSQL or execute SQL. Deterministic compiled-row decoding lives in a
separate application handoff; neither slice publishes an `IndexQueryPort`.

## Planner contract

`SchemaRegistry::plan_query` records every referenced field exactly once in
`ExecutableQueryPlan::referenced_fields`. Each `PlannedField` carries:

- the validated `FieldPath`;
- the deterministic relation alias;
- `IndexValueType`;
- field cardinality;
- nullability;
- whether the path traverses at least one many-cardinality link.

`PlannedJoin` carries the same propagated `traverses_many` boundary. The plan
fingerprint domain is `rustok-index-query-plan-v3`, so plans without canonical
many-traversal metadata cannot be confused with current compiler input.

## Supported query semantics

`SchemaRegistry::compile_postgres_query` validates and plans the query, decodes
and validates any continuation cursor, and then compiles:

- root projection and projection through explicit one-cardinality links;
- `And`, `Or`, `Not`, `Eq`, `Ne`, `In`, range, `Contains`, and `IsNull` filters;
- filtering through paths that cross one or more many-cardinality links;
- typed PostgreSQL casts for boolean, integer, decimal, string, UUID, and
  timestamp fields;
- deterministic multi-column ordering with explicit null placement;
- lexicographic keyset continuation with an ascending root `entity_id`
  tie-breaker;
- bounded cursor limits;
- bounded compatibility offset pagination;
- an optional separate exact-count statement over the same scope and filters,
  without cursor, limit, or offset leakage.

The main page statement selects hidden tagged JSONB order values when explicit
ordering is present. `decode_postgres_query_page` uses those values to construct
the next `IndexCursor` even when an order field was not requested in the public
projection.

## Predicate and ordering contract

Scalar fields are extracted from the tagged JSONB storage value and cast to the
registered field type. Field names and scalar/list values remain bind
parameters. Strings use `COLLATE "C"`; timestamps use `timestamptz`.

Atomic predicates are compiled into total booleans. Missing linked rows, absent
fields, and tagged null values do not leak SQL `NULL` into `Not`, `And`, or
`Or` semantics:

- equality, membership, range, and containment use `COALESCE(..., FALSE)`;
- root/one-link `Ne` is true only for an existing non-null scalar that differs;
- root/one-link `IsNull` treats an absent field, missing linked row, or tagged null as
  null.

Ascending order uses `NULLS LAST`; descending order uses `NULLS FIRST`, followed
by the invariant ascending `entity_id` tie-breaker. PostgreSQL/reference-engine
equivalence remains a later evidence slice rather than a claim of this source
change.

## Many-link filter boundary

Many-traversing joins are excluded from the main `FROM` clause and compiled identity
columns. Every atomic filter on such a field emits an independent nested correlated
`EXISTS` chain over `index_links` and live `index_entities` targets. Complete source
identity, source version, link name, and exact target schema identity are checked at
each hop.

Independent atomic subqueries preserve reference semantics when separate children
satisfy separate logical branches. Positive operators use any-match behavior.
`IsNull(path, true)` means no reachable non-null value exists.

Many-link `Ne` is intentionally stricter than `NOT EXISTS(Eq)`: it requires at least
one stored reachable value and also requires that no reachable value is null,
malformed, or equal to the requested value.

Because no many join enters the outer rowset, root keyset/offset pagination,
one-row lookahead, and `COUNT(*)` remain duplicate free.

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
`QueryFingerprintMismatch` before keyset SQL is emitted.

## Bind and SQL boundary

Tenant UUID, schema identities, locale, link metadata, field names, filter
values, cursor values, limit, and offset are bind values. Only fixed
`index_entities`/`index_links` table and column names plus compiler-owned `tN`,
`lN`, and `mx_*` aliases appear in SQL.

The compiler contract, SQL emission, and page/result handoff live in separate
modules:

- `application/postgres_compiler.rs` owns public SQL/bind types, scoped cursor
  entry points, and plan invariant checks;
- `application/postgres_query_sql.rs` owns controlled SQL, correlated many-link
  predicates, and deterministic bind emission;
- `application/postgres_query_result.rs` owns one-row lookahead wrapping, strict
  compiled-column decoding, exact count, `has_more`, and scoped next cursors.

## Page and result handoff

`SchemaRegistry::compile_postgres_page_query` calls the validated compiler and
changes only the main page-limit bind from `N` to `N + 1`. The SQL string,
filters, ordering, keyset predicate, offset, plan fingerprint, and optional count
statement remain unchanged.

A later database adapter executes the compiled statements and maps driver values
into `CompiledPostgresRow`. `decode_postgres_query_page` then rechecks the plan
fingerprint and complete `CompiledQueryColumn` contract, validates tagged
`IndexValue` type/cardinality/nullability, removes the lookahead row, and returns
`IndexQueryPage`. Many-filter paths do not add hidden relation columns because they
are confined to correlated subqueries.

## Exact count

`include_exact_count` produces `CompiledPostgresCount` as a separate controlled
statement with its own ordered bind list. The count applies tenant/schema/locale,
live-row, non-many outer joins, and all filters, but deliberately excludes keyset,
ordering, limit, and offset. Many filters remain correlated subqueries, so child
multiplicity cannot inflate the count.

## Fail-closed pending semantics

Projection through a many-cardinality path remains rejected with
`ManyLinkProjectionPending` until a nested aggregation result shape is explicit.
Many-link ordering remains rejected until an aggregate ordering policy exists.
Compiler validation recalculates propagated many metadata and rejects missing or
inconsistent join/field contracts before SQL emission.

## Non-claims

This source chain does not:

- prepare or execute a statement;
- convert abstract bind values into SeaORM/sqlx parameters;
- decode directly from SeaORM `QueryResult`;
- authorize callers;
- verify persisted schema readiness;
- support many-link projection or aggregate ordering;
- claim plan/SQL snapshots or PostgreSQL/reference-engine equivalence;
- read source-module tables;
- change migrations or production partition lifecycle state.

The repository owner runs formatting, compilation, tests, static verifiers, and
later PostgreSQL/reference-engine equivalence evidence.
