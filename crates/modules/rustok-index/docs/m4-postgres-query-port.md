# M4 PostgreSQL query port

This slice connects the existing typed M4 plan/compiler/result contracts to a
production PostgreSQL execution boundary. It adds source code only. No database,
Cargo, verifier, test, benchmark, or evidence command was executed for this change.

## Application port

`IndexQueryPort` is the transport-neutral owner boundary for executing an
`IndexQuery` and returning an `IndexQueryPage`.

The query already carries its tenant and locale scope. The port does not invent,
replace, or widen that scope. Authentication, caller authorization, rate limits, and
transport error mapping remain responsibilities of the server or another consuming
adapter.

`IndexQueryExecutionError` keeps validation/planning, page compilation, result
decoding, backend, persisted-schema readiness, row mapping, and storage failures
separate. Storage details are retained as typed diagnostic fields while the public
error text remains operation-level.

## PostgreSQL composition

`PostgresIndexQueryPort` owns:

- one `DatabaseConnection`;
- one immutable `Arc<SchemaRegistry>`;
- compiler and decoder access only through public Index application contracts.

The adapter rejects every backend except PostgreSQL. SQLite remains available only
for the existing storage contract tests and is not an Index query execution backend.

## Snapshot contract

One call performs the following work in one transaction:

1. begin a database transaction;
2. configure `REPEATABLE READ, READ ONLY` before any read;
3. verify persisted schema readiness;
4. execute the compiled page statement;
5. execute the optional exact-count statement;
6. map only compiler-declared columns;
7. call `SchemaRegistry::decode_postgres_query_page`;
8. commit on success or roll back on failure.

The page and exact count therefore observe one PostgreSQL snapshot. Concurrent
mutations cannot make the count describe a different committed state from the page.
The existing one-row lookahead and cursor construction remain owned by the result
decoder.

## Persisted schema readiness

Before executing compiled SQL, the adapter derives every distinct schema reference
used by the plan: the root schema plus all join source and target schemas.

For the query tenant, each exact `index_schemas` row must:

- exist;
- have status `active`;
- match the in-memory registered schema fingerprint;
- match the exact serialized schema contract.

Missing, inactive, fingerprint-drifted, or contract-drifted schemas fail closed before
the page statement executes. The check and query share the same repeatable-read
snapshot.

`index_entities` already references the complete tenant/module/entity/version/
fingerprint key in `index_schemas`, so a row cannot point at an unrelated schema
fingerprint while the database constraints are intact.

## Bind execution

`PostgresBindValue` is converted exhaustively to SeaORM values:

- boolean;
- signed integer;
- decimal;
- text;
- UUID;
- UTC timestamp;
- JSONB.

The adapter executes the exact compiler SQL and ordered bind list. It does not parse,
append, interpolate, or rewrite caller values, schema identifiers, filters, ordering,
or pagination.

## Row adapter

The page mapper reads only aliases declared by `CompiledPostgresQuery`:

- identity columns as nullable PostgreSQL UUIDs;
- projected and hidden order columns as nullable JSONB;
- nested many-relation aggregates as nullable JSONB.

The optional count mapper reads only `__exact_count` as a non-null PostgreSQL bigint.
All values are converted into `CompiledPostgresRow`; semantic validation remains in
the existing strict decoder. Missing aliases, incompatible database types, malformed
tagged values, invalid nested relation payloads, count mismatches, and cursor contract
mismatches fail closed through typed errors.

## Remaining boundaries

This slice does not:

- wire the port into `rustok-server`, storefront, admin, or `rustok-search`;
- add a transport or authorization adapter;
- add source-module schemas, mutations, rebuild sources, or consumer cutover;
- define ordering through a many-cardinality link;
- change migrations, partition lifecycle, or query SQL semantics;
- claim formatting, compilation, tests, static verifier results, live PostgreSQL
  execution, or PostgreSQL/reference-engine equivalence.

Those remain separate source and later owner-operated verification steps.
