# M4 controlled PostgreSQL query compiler

This M4 chain compiles the database-independent `ExecutableQueryPlan` into
controlled PostgreSQL statements plus ordered typed bind lists. It does not
connect to PostgreSQL, execute SQL, decode result rows, or publish an
`IndexQueryPort`.

## Planner contract

`SchemaRegistry::plan_query` now records every referenced field exactly once in
`ExecutableQueryPlan::referenced_fields`. Each `PlannedField` carries:

- the validated `FieldPath`;
- the deterministic relation alias;
- `IndexValueType`;
- field cardinality;
- nullability.

The plan fingerprint domain is `rustok-index-query-plan-v2`, so plans produced
before typed field contracts cannot be confused with the new compiler input.

## Supported query semantics

`SchemaRegistry::compile_postgres_query` validates and plans the query, decodes
and validates any continuation cursor, and then compiles:

- root projection and projection through explicit one-cardinality links;
- `And`, `Or`, `Not`, `Eq`, `Ne`, `In`, range, `Contains`, and `IsNull` filters;
- typed PostgreSQL casts for boolean, integer, decimal, string, UUID, and
  timestamp fields;
- deterministic multi-column ordering with PostgreSQL null placement matching
  the reference engine;
- lexicographic keyset continuation with an ascending root `entity_id`
  tie-breaker;
- bounded cursor limits;
- bounded compatibility offset pagination;
- an optional separate exact-count statement over the same scope and filters,
  without cursor, limit, or offset leakage.

The main page statement selects hidden tagged JSONB order values when explicit
ordering is present. A later result decoder can therefore construct the next
`IndexCursor` even when an order field was not requested in the public
projection.

## Predicate and ordering contract

Scalar fields are extracted from the tagged JSONB storage value and cast to the
registered field type. Field names and scalar/list values remain bind
parameters. Strings use `COLLATE "C"`; timestamps use `timestamptz`.

Atomic predicates are compiled into total booleans. Missing linked rows, absent
fields, and tagged null values do not leak SQL `NULL` into `Not`, `And`, or
`Or` semantics:

- equality, membership, range, and containment use `COALESCE(..., FALSE)`;
- `Ne` is true only for an existing non-null scalar that differs;
- `IsNull` treats an absent field, missing linked row, or tagged null as null.

Ascending order uses `NULLS LAST`; descending order uses `NULLS FIRST`. This
matches the existing reference comparison before the invariant ascending
`entity_id` tie-breaker.

## Cursor boundary

Opaque cursor bytes are never accepted directly by
`ExecutableQueryPlan::compile_postgres`. A continuation query must use
`SchemaRegistry::compile_postgres_query`, which verifies:

- checksum and cursor version;
- tenant, schema, locale, and schema fingerprint;
- non-nil entity identity;
- order-value arity;
- every non-null cursor order value against the registered field type.

Only then is the cursor compiled into lexicographic predicates.

## Bind and SQL boundary

Tenant UUID, schema identities, locale, link metadata, field names, filter
values, cursor values, limit, and offset are bind values. Only fixed
`index_entities`/`index_links` table and column names plus compiler-owned `tN`
and `lN` aliases appear in SQL.

The compiler contract and SQL emission live in separate modules:

- `application/postgres_compiler.rs` owns public types, cursor entry points, and
  plan invariant checks;
- `application/postgres_query_sql.rs` owns controlled SQL and deterministic bind
  emission.

## Exact count

`include_exact_count` produces `CompiledPostgresCount` as a separate controlled
statement with its own ordered bind list. The count applies tenant/schema/locale,
live-row, join, and filter predicates, but deliberately excludes keyset,
ordering, limit, and offset.

## Fail-closed pending semantics

Any path containing a many-cardinality link remains rejected with
`ManyLinkSemanticsPending`. Compiling such paths through ordinary joins would
risk duplicate roots and incorrect `Not`, `Ne`, count, projection, and
pagination semantics. A later M4 slice must introduce explicit `EXISTS` and/or
nested aggregation planning before that gate can be removed.

## Non-claims

This source slice does not:

- prepare or execute a statement;
- convert abstract bind values into SeaORM/sqlx parameters;
- decode page, projection, count, or cursor rows;
- authorize callers;
- verify persisted schema readiness;
- support many-link filtering/projection/aggregation;
- read source-module tables;
- change migrations or production partition lifecycle state.

The repository owner runs formatting, compilation, tests, static verifiers, and
later PostgreSQL/reference-engine equivalence evidence.
