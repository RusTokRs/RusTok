# Bounded external-Iggy DLQ duplicate scan

Status: **global scanner harness and fair-window source complete; runtime execution pending**.

## Purpose

`IggyDlqDuplicateScanner` adapts the transport-neutral physical duplicate classifier to an already connected external `IggyClient`.

It supports two explicit bounded questions:

```text
global request:
  within these ordered DLQ partitions, starting at one explicit offset,
  what is the count-only summary for at most N messages total?

fair snapshot window:
  within every selected DLQ partition, starting at one explicit offset,
  what is the count-only summary for at most N messages per partition?
```

It does not discover the broker, own credentials, create topology, persist a cursor, or perform reconciliation.

## Public API

```text
IggyDlqDuplicateScanRequest
IggyDlqDuplicateScanWindowPolicy
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

## Global request bounds

`IggyDlqDuplicateScanRequest` requires:

- 1 to 128 unique positive partition IDs;
- one explicit start offset applied independently to every selected partition;
- `max_messages` between 1 and 10,000;
- `batch_size` between 1 and 1,000;
- `batch_size <= max_messages`.

The message budget is global across all partitions. Partitions are scanned in caller-provided order until the global budget is exhausted. A busy earlier partition may prevent later partitions from being polled.

## Fair snapshot-window bounds

`IggyDlqDuplicateScanWindowPolicy` requires:

- 1 to 128 unique positive partition IDs;
- one explicit start offset applied independently to every selected partition;
- one positive `per_partition_messages`;
- `batch_size` between 1 and 1,000;
- `batch_size <= per_partition_messages`;
- checked `partition_count * per_partition_messages <= 10,000`.

On a successful fair-window scan, every configured partition is attempted under the same cap. All observations are combined before `summarize_dlq_duplicates`, preserving repeated deterministic IDs and conflicting payload groups that span partitions.

This is one fixed snapshot window. It does not add a moving cursor, per-partition stored progress, cross-cycle identity/digest accumulation, current-tail coverage, or complete-history proof.

A caller that needs different offsets per partition must submit separate reviewed requests. Moving independent windows without retaining bounded prior identity state can split duplicate copies across cycles and hide the relationship.

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

## Server integration

The mode-aware event-delivery observer now supports both scanner policies:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=global_budget
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=fair_window
```

`global_budget` remains the default for compatibility. `fair_window` is explicit opt-in and requires:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_PER_PARTITION_MESSAGES
```

The server integration does not change `memory` or `outbox_local`, does not create another `IggyTransport`, and cannot become a Profiles authorization input.

## Privacy boundary

The scanner temporarily passes only these values to the in-memory classifier:

```text
physical Iggy header UUID
exact physical payload bytes
```

The classifier immediately reduces payload bytes to a domain-separated SHA-256 value. The returned summary contains only counts and exposes no broker address, stream/topic/partition/offset, message UUID, payload/digest, credentials, or raw Iggy error.

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

The opt-in `dlq_duplicate_external_scan` target defines one exact compatibility-global case against a reviewed disposable external broker with message-ID deduplication disabled.

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

Fair-window external-Iggy execution evidence remains separate and pending.

## Source verification

Machine contracts:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-runtime-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-server-observer-source.json
```

Static source guards:

```bash
node scripts/verify/verify-iggy-dlq-duplicate-external-scan.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan-runtime.mjs
node scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs
```

No test, Cargo command, formatter, verifier, external Iggy connection, or runtime scan was executed while defining these slices.

## Remaining work

1. execute the compatibility-global runtime case against a reviewed dedup-disabled disposable broker;
2. add and execute a multi-partition fair-window case, including a cross-partition duplicate/conflict group;
3. retain privacy-safe packets without addresses, credentials, identifiers, payloads, offsets, or raw logs;
4. design moving windows plus bounded cross-cycle duplicate state, or keep fixed windows;
5. keep acknowledgement/delete/replay reconciliation in a separately authorized workflow;
6. preserve identifier-free aggregate correlation with poison receipt health.
