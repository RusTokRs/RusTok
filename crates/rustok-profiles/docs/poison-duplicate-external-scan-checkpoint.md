# Profiles checkpoint: bounded external DLQ duplicate scan

Status: **external-Iggy scan adapter source-complete; runtime evidence pending**.

## What changed

`rustok-iggy` now owns a bounded read-only adapter that feeds physical external-Iggy `dlq` messages into the count-only duplicate classifier.

Source:

```text
crates/rustok-iggy/src/dlq_duplicate_external_scan.rs
```

Machine contract:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-source.json
```

Verifier:

```text
scripts/verify/verify-iggy-dlq-duplicate-external-scan.mjs
```

Public API:

```text
IggyDlqDuplicateScanRequest
IggyDlqDuplicateScanner
IggyDlqDuplicateScanError
```

The result remains `DlqDuplicateSummary`; no new Profiles API or authorization input was added.

## Read-only Iggy boundary

The scanner borrows an already connected `IggyClient` and fixes:

```text
topic = dlq
consumer kind = standalone Consumer
polling strategy = explicit offset
partition = explicit positive ID
auto_commit = false
```

It does not join a consumer group, use stored-offset `next` polling, store a consumer offset, acknowledge a cursor, discover topic topology, publish, delete, purge, replay, retry, or shut down the caller's client.

## Bounded request

A request is limited to:

- at most 128 unique positive partitions;
- at most 10,000 physical messages globally;
- batches of at most 1,000 messages;
- one explicit start offset applied independently to each partition.

The scanner fails closed on response partition/count mismatch, non-monotonic offsets, offsets before the requested position, nil physical header UUID, or offset overflow.

## Count-only privacy boundary

During a scan, physical header UUIDs and exact bytes exist only long enough to create in-memory duplicate observations. Exact bytes are reduced to a domain-separated digest inside the classifier.

The returned result exposes only:

```text
total_messages
unique_message_ids
duplicate_messages
duplicate_groups
conflicting_payload_groups
max_copies_per_message_id
```

It does not expose broker addresses, stream/topic/partition/offset, UUIDs, payloads, payload digests, credentials, or raw Iggy errors.

## Profiles authorization remains unchanged

No profile visibility, relationship, block, mute, follow, friendship, audience, ownership, or presentation decision may depend on:

- whether a DLQ scan was performed;
- selected partitions or offsets;
- physical duplicate counts;
- conflicting-payload counts;
- scanner errors;
- broker deduplication configuration;
- future retained evidence metadata.

Profiles continues to consume authorized owner-port results. The scanner is operational observability for downstream neutralization only.

## Operator interpretation

A bounded scan is not automatically a complete historical inventory. Its meaning depends on the explicitly selected stream, partitions, offsets, retention window, and message cap.

Aggregate receipt health and aggregate physical duplicate health remain independent identifier-free views. They may be compared as operational trends, but they cannot be joined message by message and must not become authorization evidence.

Any `conflicting_payload_groups > 0` result requires manual forensic escalation. The scanner does not identify the affected UUID or provide a destructive action.

## Remaining work

1. add a disposable external-Iggy runtime harness with controlled duplicate/conflict fixtures;
2. prove explicit-offset polling with `auto_commit=false` does not persist progress;
3. retain only count-level runtime evidence;
4. define alert thresholds outside Profiles and outside the scanner;
5. design acknowledgement/delete/replay as a separate explicitly authorized workflow;
6. preserve the identifier-free boundary when comparing receipt and physical duplicate trends.

No tests, Cargo commands, formatters, source verifiers, external-Iggy scans, or retained capture were run by the implementation agent.
