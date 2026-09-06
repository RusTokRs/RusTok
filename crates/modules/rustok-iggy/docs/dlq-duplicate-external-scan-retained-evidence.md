# Retained external-Iggy DLQ duplicate scan evidence

Status: **execution contract, runner, and verifier source-complete; canonical runtime packet absent**.

## Purpose

This retained path executes the exact source-complete runtime case for the bounded physical DLQ duplicate scanner and stores only privacy-safe aggregate evidence.

It binds one clean Git commit to:

- a reviewed dedup-disabled external Iggy configuration;
- one exact Cargo test case;
- the required duplicate/conflict count summary;
- three required absent-offset observations;
- current source SHA-256 values;
- bounded service/toolchain metadata;
- a test-output digest and byte count.

It does not retain the broker endpoint, credentials, config path/content, message identifiers, payloads, offsets, or raw logs.

## Files

Execution contract:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-execution-contract.json
```

Capture runner:

```text
scripts/evidence/capture-iggy-dlq-duplicate-external-scan.mjs
```

Strict verifier:

```text
scripts/verify/verify-iggy-dlq-duplicate-external-scan-retained.mjs
```

Canonical packet, absent until successful execution:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-execution.json
```

## Required environment

External endpoint:

```text
RUSTOK_IGGY_DUPLICATE_SCAN_TEST_ADDRESS
```

Absolute reviewed server-config path outside the repository:

```text
RUSTOK_IGGY_DUPLICATE_SCAN_TEST_CONFIG_PATH
```

Bounded operator-reviewed Iggy version, image digest, or artifact label:

```text
RUSTOK_IGGY_DUPLICATE_SCAN_TEST_SERVER_ARTIFACT
```

Optional paired credentials:

```text
RUSTOK_IGGY_DUPLICATE_SCAN_TEST_USERNAME
RUSTOK_IGGY_DUPLICATE_SCAN_TEST_PASSWORD
```

The endpoint is validated as `host:port` and is never retained. Credentials must be both set or both empty and are never retained. Artifact metadata must not look like an endpoint.

## Reviewed configuration

The config file must be absolute, existing, and outside the repository. The runner reads only:

```toml
[system.message_deduplication]
enabled = false
```

The retained canonical configuration is only:

```json
{
  "section": "system.message_deduplication",
  "enabled": false,
  "canonical_sha256": "..."
}
```

The path, full content, unrelated settings, and full-file SHA-256 are excluded.

## Exact execution

The runner executes only:

```bash
cargo test -p rustok-iggy --features iggy \
  --test dlq_duplicate_external_scan -- \
  bounded_scan_classifies_duplicates_and_preserves_absent_consumer_offset \
  --exact --nocapture --test-threads=1
```

It requires:

- process exit status zero;
- `running 1 test`;
- exact `<case> ... ok` output;
- no source-harness skip marker.

A missing environment variable, invalid reviewed config, failed test, skip, changed commit, changed source hash, or dirty working tree produces no canonical packet.

## Clean-commit boundary

Before execution the runner requires:

- a clean tracked and untracked working tree;
- a full lowercase 40-character Git commit;
- all bound source files present.

After execution it requires:

- the same Git commit;
- the same SHA-256 for every bound source file, including runner, verifier, and execution contract;
- a still-clean working tree.

Only then is the packet written through a temporary file and atomic rename.

## Retained assertions

The packet retains the exact aggregate expectations enforced by the runtime case:

```text
total_messages = 4
unique_message_ids = 2
duplicate_messages = 2
duplicate_groups = 2
conflicting_payload_groups = 1
max_copies_per_message_id = 2
has_physical_duplicates = true
has_identity_conflicts = true
requires_manual_review = true
```

It also retains only these aggregate offset assertions:

```text
before_fixture_publication_stored_offset_present = false
after_first_scan_stored_offset_present = false
after_second_scan_stored_offset_present = false
```

These are contract assertions attached to one all-pass case. No partition, offset value, consumer identifier, message UUID, or payload is stored.

## Packet privacy

The packet permits only:

- environment-variable names;
- reviewed Iggy artifact label;
- canonical dedup-disabled values and digest;
- Git commit and timestamps;
- Cargo and Rust compiler version lines;
- exact command provenance;
- current source hashes;
- aggregate required summary and absent-offset booleans;
- `pass` result;
- test-output SHA-256 and byte count.

The strict verifier rejects forbidden delivery-level or endpoint/config fields.

## Maintainer flow

Before execution, verify source contracts:

```bash
node scripts/verify/verify-iggy-dlq-duplicate-external-scan.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan-runtime.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan-retained.mjs
```

From a clean commit with required environment supplied:

```bash
node scripts/evidence/capture-iggy-dlq-duplicate-external-scan.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan-retained.mjs
```

Review the generated packet before committing it. Re-execute whenever any bound source hash changes.

## Non-claims

This retained path does not establish active server configuration readback, complete production history, production dedup-window sufficiency, bundled mode, TLS/auth/failover, multi-partition runtime, destructive reconciliation, or Profiles authorization.

No test, Cargo command, formatter, verifier, external-Iggy connection, or retained capture was executed while defining this tooling.
