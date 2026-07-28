# Retaining Social Graph raw poison PostgreSQL/Iggy evidence

## Status

The retained execution contract, capture runner, and strict verifier are source-complete. The canonical execution packet is intentionally absent until a maintainer executes both combined ordering cases successfully.

## Files

- execution contract: `crates/rustok-social-graph/contracts/evidence/index-raw-poison-postgres-iggy-execution-contract.json`
- capture runner: `scripts/evidence/capture-social-graph-index-raw-poison-postgres-iggy.mjs`
- retained verifier: `scripts/verify/verify-social-graph-index-raw-poison-postgres-iggy-retained.mjs`
- future packet: `crates/rustok-social-graph/contracts/evidence/index-raw-poison-postgres-iggy-execution.json`

The source harness and its production-order guard remain documented in `index-raw-poison-postgres-iggy-evidence.md`.

## Required inputs

```text
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_DATABASE_URL=postgresql://...
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_ADDRESS=host:8090
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_POSTGRES_ARTIFACT=<reviewed version/image label>
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_SERVER_ARTIFACT=<reviewed version/image label>
```

Optional Iggy credentials must be supplied together:

```text
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_USERNAME=...
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_PASSWORD=...
```

The runner validates that the database URL uses PostgreSQL and that the Iggy endpoint is a bounded `host:port` value without embedded credentials or URL parameters. It stores only the names of the URL/address environment variables, never their values.

Artifact labels are bounded one-line operator-reviewed metadata. They are not active server readback and must not be described as such.

## Execution gates

The runner requires:

- a clean Git working tree before execution;
- one full lowercase commit SHA;
- current SHA-256 values for all bound production and test sources;
- Cargo and Rust compiler version metadata;
- both required services and both reviewed artifact labels;
- paired optional Iggy credentials;
- each named case to run independently through a libtest `--exact` filter;
- `running 1 test` and `<case> ... ok` for each command;
- no source-harness skip marker;
- unchanged commit, source hashes, and clean tree after execution.

The packet is written atomically only after both cases pass.

## Exact commands

The runner constructs these commands from the locked template:

```bash
cargo test -p rustok-social-graph --features index-consumer \
  --test index_raw_poison_postgres_iggy -- \
  raw_poison_persists_published_before_source_acknowledgement \
  --exact --nocapture --test-threads=1

cargo test -p rustok-social-graph --features index-consumer \
  --test index_raw_poison_postgres_iggy -- \
  published_redelivery_is_acknowledgement_only_without_republication \
  --exact --nocapture --test-threads=1
```

## Retained packet

The packet retains only:

- commit and timestamps;
- Cargo/Rust compiler versions;
- bounded reviewed PostgreSQL and Iggy artifact labels;
- environment-variable names for the database URL and Iggy address;
- exact command arrays;
- source SHA-256 values;
- per-case and combined test-output SHA-256 values and byte counts;
- two aggregate `pass` results and their locked assertion names.

It does not retain:

- the database URL or Iggy address;
- usernames, passwords, or connection strings;
- raw test output;
- PostgreSQL schema or Iggy stream names;
- payloads, source offsets, acknowledgement tokens, or delivery UUIDs.

## Verifier modes

Before execution JSON exists, the verifier checks the contract, runner, source-contract state, exact commands, clean-commit gates, hashing, atomic output, and packet privacy projection. It reports runtime pending.

After execution JSON exists, it additionally requires:

- packet commit equal to current `HEAD`;
- current source SHA-256 values;
- valid bounded timestamps, toolchain values, and artifact labels;
- exactly two retained cases in contract order;
- `pass` for both cases;
- exact command and assertion arrays;
- valid per-case and combined output hashes and positive byte counts;
- no forbidden packet keys.

## Maintainer flow

```bash
node scripts/verify/verify-social-graph-index-raw-poison-postgres-iggy.mjs
node scripts/verify/verify-social-graph-index-raw-poison-postgres-iggy-retained.mjs

# Supply the required inputs, then:
node scripts/evidence/capture-social-graph-index-raw-poison-postgres-iggy.mjs
node scripts/verify/verify-social-graph-index-raw-poison-postgres-iggy-retained.mjs
```

Review the generated packet before committing it.

## Non-claims

A successful retained packet proves the two bounded ordering scenarios for the reviewed service artifacts and bound source commit. It does not prove a PostgreSQL/Iggy transaction, physical exactly-once, production dedup-window sufficiency, bundled mode, TLS/auth/failover, multi-replica ownership, or Profiles authorization.

No Cargo command, verifier, database query, broker scenario, formatter, or retained capture was executed while authoring this tooling.
