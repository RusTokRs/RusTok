# Retained publish/mark ambiguity evidence

Status: **execution contract locked; canonical runtime packet absent**.

This retained path promotes the source harness in `index_raw_poison_publish_mark_ambiguity.rs` only after both PostgreSQL + external-Iggy scenarios pass from one clean commit.

## Required inputs

### PostgreSQL

```text
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_DATABASE_URL=postgresql://...
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_POSTGRES_ARTIFACT=postgresql-16.4-reviewed-build
```

The artifact value is a bounded operator-reviewed version, image digest, or build label. It must not be an endpoint. The database URL is validated but never retained.

### Dedup-enabled Iggy

```text
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_DEDUP_ENABLED_ADDRESS=host:port
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_DEDUP_ENABLED_CONFIG_PATH=/outside/repository/enabled.toml
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_DEDUP_ENABLED_SERVER_ARTIFACT=iggy-server-reviewed-build
```

The reviewed configuration must contain:

```toml
[system.message_deduplication]
enabled = true
max_entries = 1 # or greater
expiry = "2s"  # strictly greater than the 1500 ms recovery wait
```

A larger production value is expected. The retained gate only requires enough reviewed capacity and expiry for the exact isolated scenario.

### Dedup-disabled Iggy

```text
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_DEDUP_DISABLED_ADDRESS=host:port
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_DEDUP_DISABLED_CONFIG_PATH=/outside/repository/disabled.toml
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_DEDUP_DISABLED_SERVER_ARTIFACT=iggy-server-reviewed-build
```

The reviewed configuration must contain:

```toml
[system.message_deduplication]
enabled = false
```

The two addresses and the two config paths must be distinct. Config paths must be absolute, must identify existing files, and must remain outside the repository.

### Optional credentials

```text
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_IGGY_USERNAME=...
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_IGGY_PASSWORD=...
```

Both must be supplied together. Their values are validated for the source harness boundary and are never retained.

## Reviewed configuration projection

The runner reads only `[system.message_deduplication]` and stores the canonical non-secret projection:

```text
enabled
max_entries
expiry
expiry_milliseconds
canonical_sha256
```

It does not retain:

- config paths;
- full config contents;
- full-file hashes;
- unrelated Iggy settings;
- addresses or credentials.

## Exact execution

Each scenario is run separately:

```bash
cargo test -p rustok-social-graph --features index-consumer \
  --test index_raw_poison_publish_mark_ambiguity -- \
  dedup_enabled_closes_publish_mark_ambiguity_without_physical_duplicate \
  --exact --nocapture --test-threads=1
```

```bash
cargo test -p rustok-social-graph --features index-consumer \
  --test index_raw_poison_publish_mark_ambiguity -- \
  dedup_disabled_exposes_publish_mark_ambiguity_as_physical_duplicate \
  --exact --nocapture --test-threads=1
```

For each command the runner requires:

- exit status zero;
- `running 1 test`;
- the exact named case followed by `... ok`;
- no source-harness skip message.

## Clean-commit capture

Run the static source and retained verifiers before capture:

```bash
node scripts/verify/verify-social-graph-index-raw-poison-publish-mark-ambiguity.mjs
node scripts/verify/verify-social-graph-index-raw-poison-publish-mark-ambiguity-retained.mjs
```

Then capture:

```bash
node scripts/evidence/capture-social-graph-index-raw-poison-publish-mark-ambiguity.mjs
```

Finally verify the generated packet:

```bash
node scripts/verify/verify-social-graph-index-raw-poison-publish-mark-ambiguity-retained.mjs
```

The runner requires a clean working tree before execution and rechecks:

- full commit SHA;
- current source SHA-256 values, including the contracts, runner, and verifiers;
- unchanged working tree after both tests;
- bounded toolchain metadata.

The canonical packet is written atomically only after both exact cases pass.

## Retained packet

Canonical path:

```text
crates/rustok-social-graph/contracts/evidence/index-raw-poison-publish-mark-ambiguity-execution.json
```

The packet retains:

- commit and timestamps;
- Cargo and Rust compiler versions;
- environment-variable names for PostgreSQL and Iggy inputs;
- reviewed PostgreSQL/Iggy artifact labels;
- canonical dedup values and digests;
- current source hashes;
- exact command arrays;
- expected physical count sequences;
- per-case and combined output SHA-256 values and byte counts;
- two aggregate `pass` results.

The packet omits database URLs, broker addresses, config paths, credentials, connection strings, full configs, raw logs, payloads, offsets, delivery UUIDs, acknowledgement tokens, schema names, and stream names.

## Interpretation

A successful retained packet establishes the exact reviewed configuration and isolated runtime behavior:

```text
dedup enabled:  0 -> 1 -> 1
dedup disabled: 0 -> 1 -> 2
```

It does not establish a PostgreSQL/Iggy transaction or universal production exactly-once behavior. The enabled result applies only while the deterministic ID remains inside the reviewed broker expiry and capacity window. Longer outages, capacity pressure, failover, and multi-replica operation require separate operational evidence.

The execution JSON is intentionally absent until a maintainer runs the capture successfully. No tests, Cargo commands, formatters, verifiers, PostgreSQL, or Iggy scenarios were run while adding this retained path.
