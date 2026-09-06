# Consumer poison receipt PostgreSQL evidence

## Scope

`consumer_poison_receipt_postgres.rs` is an opt-in PostgreSQL integration harness for the connector-owned neutral receipt store and count-only inspector. It exercises database behavior that SQLite source tests cannot establish:

- concurrent publication claim ownership across independent PostgreSQL connections;
- lease expiry, reclaim, and fencing of the previous publisher;
- deterministic delivery UUID and source-coordinate collision rollback;
- retention of the first bounded error classification and delivery-attempt diagnostic;
- empty exact payload acceptance;
- terminal `published` and `acknowledged` recognition;
- aggregate inspector consistency across reserved and terminal states.

The harness does not connect to Iggy, publish a DLQ entry, acknowledge a source cursor, choose retry policy, authorize a profile, or delete retained receipts.

## Database selection

Set one of:

1. `RUSTOK_IGGY_CONNECTOR_TEST_DATABASE_URL`;
2. `DATABASE_URL` as a fallback.

The value must begin with `postgres://` or `postgresql://`. Without a PostgreSQL URL, the direct test harness reports a skip and returns successfully. The retained-evidence runner rejects a missing or non-PostgreSQL URL and writes no packet. The repository contains no default PostgreSQL credentials or localhost fallback.

## Isolation

Each scenario:

1. creates a unique schema named `rustok_iggy_poison_<scenario>_<uuid>`;
2. opens an isolated one-connection pool;
3. sets `search_path` to the unique schema;
4. applies connector migrations directly through `SchemaManager`;
5. runs the scenario;
6. drops the schema with `CASCADE`.

The concurrent claim test opens two additional one-connection pools and sets the same isolated search path on both. This is deliberate: `SET search_path` is session-local, so a multi-connection pool configured through one session would not prove schema confinement.

## Scenarios

### Claim ownership

Two publishers call `reserve_and_claim` concurrently for the same exact delivery. Exactly one result must be `Claimed`; the other must be `Busy`. The durable row remains `publishing` and retains one atomic first-observed error-code/attempt pair.

### Lease reclaim and fencing

The first publisher claims an empty payload. Test-only SQL expires its lease deterministically. A second publisher reclaims the row. The first publisher must receive `ClaimLost` when attempting `mark_published`; the second publisher can mark the receipt terminal. The first diagnostic remains unchanged.

Direct SQL is limited to this deterministic lease-expiry operation and read-only count diagnostics. Production state transitions remain exercised through `ConsumerPoisonReceiptStore`.

### Collision rollback

The harness verifies both collision directions:

- one deterministic delivery UUID reused for different source coordinates;
- one source coordinate reused for different delivery UUID or bytes.

Both fail as `IdentityConflict`. The original receipt remains unchanged and the table contains exactly one row.

### Terminal aggregate consistency

The harness creates one reserved, one published, and one acknowledged receipt. `ConsumerPoisonReceiptInspector` must report total `3` with exact per-state counts and no expired publishing claims. Redelivery must return `AlreadyPublished` or `AlreadyAcknowledged` without reopening publication.

## Retained execution contract

`contracts/evidence/consumer-poison-postgres-execution-contract.json` locks:

- the two exact Cargo commands;
- the four required scenario names and assertions;
- the source files whose SHA-256 values bind an executed packet;
- required Git, toolchain, PostgreSQL, timestamp, and output-digest metadata;
- the output path for retained evidence.

`capture-iggy-consumer-poison-postgres.mjs` requires a clean working tree and a full Git commit SHA. It executes a small PostgreSQL environment test followed by the receipt scenarios with one test thread. The environment test reads `server_version` and `server_version_num` through the same SeaORM/PostgreSQL path; no `psql` installation is required.

The runner writes `consumer-poison-postgres-execution.json` atomically only when both Cargo commands succeed, every required test reports `ok`, the Git commit remains unchanged, and source hashes remain stable throughout execution.

The retained packet contains no database URL, host, database name, credentials, raw test log, delivery UUID, source coordinate, or payload. It stores only:

- the environment variable name that supplied the URL;
- bounded PostgreSQL and Rust toolchain versions;
- start/end timestamps and Git commit;
- exact command arrays;
- source and combined-output SHA-256 values;
- aggregate per-case `pass` results.

## Maintainer commands

Diagnostic harness only:

```bash
RUSTOK_IGGY_CONNECTOR_TEST_DATABASE_URL='postgresql://…' \
  cargo test -p rustok-iggy-connector --features migrations \
  --test consumer_poison_receipt_postgres -- --nocapture
```

Create the retained packet from a clean commit:

```bash
RUSTOK_IGGY_CONNECTOR_TEST_DATABASE_URL='postgresql://…' \
  node scripts/evidence/capture-iggy-consumer-poison-postgres.mjs

node scripts/verify/verify-iggy-consumer-poison-retained-evidence.mjs
```

The existing source harness guard remains available:

```bash
node scripts/verify/verify-iggy-consumer-poison-postgres-evidence.mjs
```

For stronger concurrency evidence, execute the retained runner repeatedly against the same PostgreSQL server on separate clean commits or retain separately reviewed packets outside the canonical repository path. Do not reuse a schema or replace the unique-schema setup with shared-table truncation.

## Evidence status

The harness, execution contract, metadata test, runner, and source verifiers are source-complete. `consumer-poison-postgres-execution.json` is intentionally absent until a maintainer performs a successful PostgreSQL run. No PostgreSQL command, Cargo command, formatter, or source verifier was executed when this slice was authored.
