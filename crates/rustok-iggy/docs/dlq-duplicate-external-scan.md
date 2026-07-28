# Bounded external-Iggy DLQ duplicate scan

Status: **scanner and disposable-broker harness source-complete; runtime execution pending**.

## Purpose

`IggyDlqDuplicateScanner` adapts the transport-neutral physical duplicate classifier to an already connected external `IggyClient`.

It answers one bounded operational question:

```text
Within these explicit DLQ partitions, starting at this explicit offset,
what is the count-only physical duplicate summary for at most N messages?
```

It does not discover the broker, own credentials, create topology, persist a cursor, or perform reconciliation.

## Public API

```text
IggyDlqDuplicateScanRequest
IggyDlqDuplicateScanner
IggyDlqDuplicateScanError
```

The result is the existing identifier-free:

```text
DlqDuplicateSummary
```

Connection and authentication lifecycle remain owned by the caller. The scanner borrows an already connected `IggyClient` and never calls shutdown.

## Polling boundary

The adapter is deliberately lower level than an `IggyConsumer` stream:

```text
consumer kind: standalone Consumer
consumer name: rustok-dlq-duplicate-readonly-v1
topic: dlq
partition: explicit positive ID
strategy: PollingStrategy::offset(explicit_offset)
auto_commit: false
```

A regular consumer requires the partition to be supplied explicitly. The scanner does not use a consumer group and does not use `PollingStrategy::next`, because `next` depends on stored consumer offsets.

The scanner also avoids `get_topic` and `get_topics`. Operators must supply the intended partition allowlist, so the account needs message-poll permission rather than topic-management or topic-discovery behavior from this adapter.

## Request bounds

One request requires:

- 1 to 128 unique positive partition IDs;
- one explicit start offset applied independently to every selected partition;
- `max_messages` between 1 and 10,000;
- `batch_size` between 1 and 1,000;
- `batch_size <= max_messages`.

The message budget is global across all partitions. Partitions are scanned in caller-provided order until the global budget is exhausted.

A caller that needs different offsets per partition must submit separate requests. This avoids silently inventing or retaining a partition cursor map.

## Response validation

Each physical poll response must satisfy all of the following:

- returned partition equals the requested partition;
- reported count equals the number of returned messages;
- reported count does not exceed the requested batch;
- every message offset is at or after the explicit requested offset;
- offsets in one batch are strictly increasing;
- advancing `last_offset + 1` does not overflow;
- every physical header ID is a non-nil UUID accepted by the classifier.

Any mismatch fails closed. Raw client errors and broker coordinates are not copied into the public error.

## Privacy boundary

The scanner temporarily passes only these values to the in-memory classifier:

```text
physical Iggy header UUID
exact physical payload bytes
```

The classifier immediately reduces payload bytes to a domain-separated SHA-256 value. The returned summary contains only counts and exposes no:

- broker address;
- stream or topic name;
- partition or offset;
- message UUID;
- payload or payload digest;
- credentials;
- raw Iggy error.

The scanner error codes are bounded and identifier-free:

```text
iggy.dlq_duplicate.scan_invalid
iggy.dlq_duplicate.scan_failed
iggy.dlq_duplicate.scan_response_invalid
iggy.dlq_duplicate.scan_offset_overflow
```

Classifier identity/count errors retain their existing stable codes.

## Mutation boundary

The adapter contains no call to:

- automatic offset commit;
- consumer offset storage;
- acknowledgement or high-level cursor offset storage;
- stream/topic deletion or purge;
- message publication;
- DLQ replay or retry;
- poison receipt claim, release, publish, or acknowledgement transitions;
- client shutdown.

Polling uses `auto_commit = false`. The fixed consumer identifier is therefore only the request identity supplied to Iggy; the adapter does not persist progress for later `next` polling.

## Source-complete runtime harness

The opt-in `dlq_duplicate_external_scan` test target now defines one exact case against a reviewed disposable external broker with message-ID deduplication disabled.

It publishes through production `IggyTransport::move_to_dlq`:

```text
A, A: same header UUID and same exact bytes
B1, B2: same header UUID and different exact bytes
```

The same `[partition 1, offset 0, max 4, batch 4]` scan is executed twice. Both summaries must equal:

```text
total_messages = 4
unique_message_ids = 2
duplicate_messages = 2
duplicate_groups = 2
conflicting_payload_groups = 1
max_copies_per_message_id = 2
```

Read-only `get_consumer_offset` must return `None` before publication, after the first scan, and after the second scan. The test contains no direct SDK producer or offset mutation.

See `dlq-duplicate-external-scan-runtime-evidence.md` for prerequisites and the exact command.

## Suggested caller flow

```text
1. Operator selects one external service and an explicit DLQ stream.
2. Operator supplies an explicit partition allowlist and bounded offset/count window.
3. A separately configured component connects and authenticates IggyClient.
4. IggyDlqDuplicateScanner borrows the connected client.
5. Scanner polls explicit offsets with auto_commit=false.
6. Scanner returns DlqDuplicateSummary only.
7. Caller closes the client through its own lifecycle.
```

Do not present this scanner as a complete historical inventory unless the selected partitions, offsets, retention window, and message cap cover the intended history.

## Source verification

Machine contracts:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-runtime-source.json
```

Static source guards:

```bash
node scripts/verify/verify-iggy-dlq-duplicate-external-scan.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan-runtime.mjs
```

Focused source tests are embedded in `dlq_duplicate_external_scan.rs` for request bounds and stable error projection. The opt-in integration target defines the physical duplicate/conflict and absent-offset case.

No test, Cargo command, formatter, verifier, external Iggy connection, or runtime scan was executed while defining these slices.

## Remaining work

1. execute the exact runtime case against a reviewed dedup-disabled disposable broker;
2. retain a privacy-safe runtime packet without addresses, credentials, identifiers, payloads, offsets, or raw logs;
3. define alert thresholds outside the scanner;
4. keep acknowledgement/delete/replay reconciliation in a separately authorized workflow;
5. preserve identifier-free aggregate correlation with poison receipt health.
