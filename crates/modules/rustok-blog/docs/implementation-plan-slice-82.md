# rustok-blog implementation plan — slice 82 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-81.md`.

Slices 1–81 retain the typed Comments remote boundary, bounded TCP framing and
listener lifecycle, authenticated service reads, signed user-write delegation,
overlapping scheduled keyrings, one process-local replay gate, an explicitly
authorized mutation trigger, bounded process-local audit, a canonical
secret-inclusive schedule digest, and a host-owned persist-before-publish
compare-and-store contract.

## 2026-08-01 continuation audit

Slice 81 intentionally stopped at an abstract persistence contract. A deployment
could not claim restart rollback prevention until one concrete backend provided:

- exact source/generation/digest comparison;
- atomic bootstrap and replacement;
- durable commit before success;
- definitive handling of an ambiguous commit result;
- no returned error after a possibly committed write.

Slice 82 adds one concrete PostgreSQL adapter and one Blog-owned migration. The
adapter preserves the synchronous slice-81 trigger boundary by isolating async
SeaORM work on one dedicated current-thread Tokio runtime owned by a worker
thread.

Tests, source verifiers, formatting, Cargo commands, PostgreSQL execution,
workflows, and CI remain intentionally unexecuted by request.

## Slice 82 — PostgreSQL schedule persistence adapter

### Module-owned singleton table

The Blog migration
`m20260801_000007_create_blog_comments_delegation_schedule_state` creates:

```text
blog_comments_tcp_delegation_schedule_state
```

The table contains one fixed row identified by:

```text
comments_tcp_delegation_schedule
```

Columns are:

- `state_key VARCHAR(64)` primary key;
- `schema_version SMALLINT`;
- `source VARCHAR(16)`;
- `generation BIGINT`;
- `schedule_digest_hex VARCHAR(64)`;
- `updated_at TIMESTAMPTZ`.

Migration checks require schema version 1, a positive generation, a 64-character
digest encoding, and source category `host_provided` or `file`.

The table stores no key IDs, secret values, schedule JSON, credentials, nonces,
tokens, actor identity, request identity, file path, or raw backend error.

The migration is intentionally irreversible. A routine down migration must not
silently erase the accepted generation baseline.

### Concrete adapter

`PostgresCommentsTcpDelegationSchedulePersistenceStore` implements the existing
`CommentsTcpDelegationSchedulePersistenceStore`.

Construction requires an existing SeaORM `DatabaseConnection` whose backend is
PostgreSQL. SQLite and MySQL are rejected before the worker starts.

The adapter exports `into_shared()` for direct use by
`SharedCommentsTcpDelegationPersistedScheduleTrigger`.

No environment variable, global database singleton, connection URL, or
credential is accepted by this owner. Connection creation and credential
ownership remain with the server host.

### Synchronous trigger / async database bridge

The slice-81 trigger and persist-before-publish callback are synchronous because
the schedule write lock must remain held while durable CAS completes.

The PostgreSQL adapter therefore owns:

- one dedicated OS thread;
- one current-thread Tokio runtime;
- one bounded synchronous command channel with capacity 1;
- one response channel per operation.

The worker receives only complete persistence records. It never receives the
schedule document or secret material.

Trigger callers synchronously wait for a definitive database result. Schedule
replacement is a rare control-plane operation; this adapter is not a
high-throughput request-path store.

### Exact resume verification

`verify_current(expected)` reads the singleton row and compares:

- schema version;
- source category;
- generation;
- lowercase SHA-256 digest hex.

A missing row, different source, different generation, different digest, or
invalid stored representation returns `Conflict`. Database unavailability
returns `Unavailable`.

### Bootstrap CAS

For `compare_and_store(None, candidate)`, the adapter executes inside a
PostgreSQL transaction:

```sql
INSERT ... ON CONFLICT (state_key) DO NOTHING
```

Exactly one affected row is required. An existing singleton row returns
`Conflict`.

### Replacement CAS

For `compare_and_store(Some(expected), candidate)`, the adapter performs one
predicate update:

```sql
UPDATE ...
SET schema_version = candidate.schema_version,
    source = candidate.source,
    generation = candidate.generation,
    schedule_digest_hex = candidate.schedule_digest_hex,
    updated_at = NOW()
WHERE state_key = fixed_state_key
  AND schema_version = expected.schema_version
  AND source = expected.source
  AND generation = expected.generation
  AND schedule_digest_hex = expected.schedule_digest_hex
```

Exactly one affected row is required. Zero rows means another process already
created, removed, modified, or advanced the record and returns `Conflict`.

The complete expected record participates in the predicate; generation alone is
not sufficient.

### Transaction and commit ordering

The adapter:

1. begins a PostgreSQL transaction;
2. executes the insert or update CAS;
3. rolls back and returns a definitive error when execution fails or affects an
   unexpected row count;
4. commits only after exactly one row is affected;
5. returns success only after commit acknowledgement or successful
   post-error reconciliation.

Because slice 81 invokes the adapter while the schedule write lock is held, the
ordering remains:

```text
schedule invariant validation
→ PostgreSQL transaction/CAS
→ PostgreSQL commit or definitive reconciliation
→ in-memory schedule snapshot assignment
→ bounded process-local audit outcome
```

No application-level read-then-write gap exists in the PostgreSQL CAS.

### Ambiguous commit reconciliation

A connection can fail while PostgreSQL is committing, after the server may have
made the transaction durable but before the client receives acknowledgement.

Returning `Unavailable` immediately would violate the slice-81 contract because
the durable state might already contain the candidate while the old keyring
continues signing.

After a commit error, the adapter repeatedly reads the singleton row:

- exact candidate record: treat the operation as successful;
- exact expected record, or no row for an empty bootstrap expectation: return
  definitive `Unavailable`;
- any third state: fail-stop the process;
- persistence remaining unreadable after 20 attempts spaced by 100 ms:
  fail-stop the process.

Fail-stop uses `std::process::abort()`. This is deliberate: an unreconciled
commit cannot safely return control to a process that still owns the previous
signing snapshot.

There is no panic recovery, permissive fallback, stale-snapshot continuation, or
automatic baseline reset for an indeterminate commit.

### Multi-process CAS scope

The PostgreSQL row predicate provides a concrete database serialization point
for multiple processes using the same database and state key. Only one process
can successfully advance a given exact expected record.

This source-level property does not establish executed multi-replica evidence,
clock synchronization, coordinated activation, or shared replay protection.

### Representation bounds

The PostgreSQL adapter stores generation as signed `BIGINT`. A persistence
record whose generation cannot be represented as `i64` returns `Unavailable`
without issuing a database write.

The canonical schedule digest remains the exact slice-81 32-byte SHA-256 value;
the database stores its lowercase 64-character hexadecimal encoding.

### Preserved owners

The following slice-79 through slice-81 owners remain byte-for-byte unchanged:

- schedule lifecycle base;
- runtime-policy schedule guard;
- process-local authorized trigger;
- Comments lifecycle schedule;
- persistence document/digest/store contract;
- persist-before-publish bridge;
- persisted trigger.

The adapter is additive. It does not alter delegation signatures, key
activation, retirement, overlap, TTL/skew policy, replay admission, TCP framing,
listener lifecycle, channel selection, or loopback publication.

### Explicit non-claims

Slice 82 does not claim:

- executed PostgreSQL migration or query evidence;
- crash-injection evidence;
- network-partition evidence;
- proof that a particular PostgreSQL deployment acknowledges durable commits
  according to its storage configuration;
- durable audit or a transactional audit outbox;
- automatic repair of a deleted or externally modified persistence row;
- automatic schedule recovery after fail-stop;
- coordinated activation clocks across replicas;
- shared, durable, multi-replica, or restart-safe replay protection;
- an HTTP, GraphQL, native RPC, MCP, CLI, signal, watcher, or polling trigger;
- cloud secret-manager, KMS, HSM, or sidecar integration;
- secret zeroization or locked memory;
- TLS/mTLS or non-loopback publication;
- successful compilation, tests, source-verifier execution, formatting,
  PostgreSQL execution, workflows, CI, or production validation.

Status: `source_verified_no_compile`.
Compile policy: `not_run_by_request`.
Runtime status: `not_run`.

## Next implementation results

1. Add a transactionally coupled durable authorization/audit outbox record to
   the same PostgreSQL CAS transaction.
2. Add isolated PostgreSQL execution for bootstrap, exact resume, stale CAS,
   concurrent CAS, and commit-reconciliation paths.
3. Define an operator recovery ceremony for fail-stop, lost state, corruption,
   or externally advanced state.
4. Add clock-health and maximum-drift ownership before coordinated activation
   across replicas.
5. Replace process-local replay admission before claiming restart-safe or
   multi-replica replay protection.

## Suggested verification — intentionally not run

- `node scripts/verify/verify-blog-comments-tcp-delegation-schedule-postgres.mjs`
- `node scripts/verify/verify-blog-comments-tcp-delegation-schedule-persistence.mjs`
- `cargo test -p rustok-server --features mod-blog comments_provider_runtime`
- `cargo test -p rustok-blog migrations`
- `cargo check -p rustok-server --features mod-blog --locked`

## Ownership retained

- Comments owns lifecycle validation, effective keyring selection, signing,
  verification, request binding, and process-local replay admission.
- The server host owns the PostgreSQL adapter, canonical persistence record
  composition, mutation authorization, process-local audit, runtime provider
  composition, listener lifecycle, concurrency, and shutdown.
- PostgreSQL owns transactional row serialization and commit durability
  according to the configured deployment.
- Blog owns the persistence table migration and remains transport-neutral for
  authenticated rendering and degraded presentation.
