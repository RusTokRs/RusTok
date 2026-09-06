# External-Iggy DLQ duplicate scan runtime evidence

Status: **source complete; runtime execution pending**.

## Purpose

`dlq_duplicate_external_scan.rs` is an opt-in disposable-broker harness for the bounded external scanner. It proves the source shape for three related claims in one isolated one-partition stream:

1. repeated deterministic header UUID plus repeated exact bytes is reported as an ordinary physical duplicate;
2. repeated deterministic header UUID plus different exact bytes is reported as an identity conflict;
3. two explicit-offset scans with `auto_commit=false` leave the standalone scanner consumer without a stored offset.

The harness does not claim production history completeness, production dedup-window sufficiency, destructive reconciliation, or Profiles authorization.

## Broker requirement

Use one reviewed disposable or operator-cleaned external Iggy service with:

```toml
[system.message_deduplication]
enabled = false
```

The harness does not read back the server configuration. This is an operator-reviewed prerequisite. If deduplication is enabled, repeated deterministic header UUIDs may be suppressed and the required four-message result will not be present.

The harness creates a unique stream with:

```text
domain partitions = 1
DLQ partitions = 1
replication factor = 1
```

It does not delete the stream. Use a disposable service or perform separate operator cleanup.

## Environment

Required:

```text
RUSTOK_IGGY_DUPLICATE_SCAN_TEST_ADDRESS
```

Optional paired credentials:

```text
RUSTOK_IGGY_DUPLICATE_SCAN_TEST_USERNAME
RUSTOK_IGGY_DUPLICATE_SCAN_TEST_PASSWORD
```

The address must be bounded `host:port` without a scheme, embedded credentials, query, or fragment. There is no default address or credential fallback. TLS, bundled mode, and failover are outside this harness.

## Fixture publication

All four physical fixtures are published through production:

```text
IggyTransport::move_to_dlq
```

The direct SDK client is not a producer.

The fixture set is:

```text
A1: header UUID A, bytes A
A2: header UUID A, bytes A
B1: header UUID B, bytes B1
B2: header UUID B, bytes B2
```

`A1/A2` define one ordinary duplicate group. `B1/B2` define one conflicting-payload group.

The test source contains the bytes and generated UUIDs only in process memory. It does not print them or retain them.

## Bounded scans

The same scanner request is executed twice:

```text
partitions = [1]
start_offset = 0
max_messages = 4
batch_size = 4
```

Both scans must return exactly:

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

The second summary must equal the first summary. This shows that explicit-offset scanning is repeatable for the same retained physical window; it does not claim that the selected window covers complete history.

## Stored-offset non-mutation

The harness recreates the scanner's exact standalone consumer identity:

```text
consumer kind = Consumer
consumer name = rustok-dlq-duplicate-readonly-v1
partition = 1
```

It calls read-only `get_consumer_offset`:

1. before fixture publication;
2. after the first scan;
3. after the second scan.

Every observation must be `None`.

The harness never calls:

- `store_consumer_offset`;
- `delete_consumer_offset`;
- high-level cursor `store_offset`;
- acknowledgement;
- a consumer-group cursor;
- direct SDK publication;
- stream/topic deletion or purge.

The scanner source separately locks `auto_commit = false` on low-level `poll_messages`.

## Exact command

```bash
RUSTOK_IGGY_DUPLICATE_SCAN_TEST_ADDRESS='host:8090' \
  cargo test -p rustok-iggy --features iggy \
  --test dlq_duplicate_external_scan -- \
  bounded_scan_classifies_duplicates_and_preserves_absent_consumer_offset \
  --exact --nocapture --test-threads=1
```

When the required address is absent, the source harness reports a skip and returns successfully. A future retained runner must reject that skip and require the exact case to execute.

## Source contracts and guards

Runtime source contract:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-runtime-source.json
```

Runtime source verifier:

```bash
node scripts/verify/verify-iggy-dlq-duplicate-external-scan-runtime.mjs
```

The parent scanner contract and guard remain:

```bash
node scripts/verify/verify-iggy-dlq-duplicate-external-scan.mjs
```

## Privacy boundary

This source harness asserts only the count-level summary. It does not publish or retain:

- broker address or credentials;
- generated stream name;
- partition offsets;
- physical header UUIDs;
- payload bytes or payload digests;
- raw Iggy errors or logs.

A future retained execution packet must keep the same boundary and store only reviewed service metadata, source/output hashes, exact command provenance, and aggregate pass results.

## Remaining work

1. execute the exact case against a reviewed dedup-disabled disposable service;
2. add a clean-commit retained execution contract, runner, and strict verifier;
3. retain only count-level evidence and absent-offset aggregate results;
4. define alert thresholds outside the scanner;
5. keep acknowledgement/delete/replay reconciliation in a separate authorized workflow.

No test, Cargo command, formatter, verifier, external-Iggy connection, or scan was executed while defining this source slice.
