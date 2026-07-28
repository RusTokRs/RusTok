# Profiles checkpoint: bounded external DLQ duplicate scan

Status: **external-Iggy scanner and runtime harness source-complete; execution and retained evidence pending**.

## What changed

`rustok-iggy` owns a bounded read-only adapter that feeds physical external-Iggy `dlq` messages into the count-only duplicate classifier. It now also owns one opt-in disposable-broker harness proving the expected source shape for duplicate/conflict classification and absent stored offsets.

Sources:

```text
crates/rustok-iggy/src/dlq_duplicate_external_scan.rs
crates/rustok-iggy/tests/dlq_duplicate_external_scan.rs
```

Machine contracts:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-runtime-source.json
```

Verifiers:

```text
scripts/verify/verify-iggy-dlq-duplicate-external-scan.mjs
scripts/verify/verify-iggy-dlq-duplicate-external-scan-runtime.mjs
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

## Source-complete runtime case

The opt-in runtime case requires one reviewed dedup-disabled disposable external service. It publishes four physical messages through production `IggyTransport::move_to_dlq`:

```text
A, A: one ordinary duplicate group
B1, B2: one conflicting-payload group
```

Two scans reuse the exact request `[partition 1, offset 0, max 4, batch 4]`. Both must return:

```text
total_messages = 4
unique_message_ids = 2
duplicate_messages = 2
duplicate_groups = 2
conflicting_payload_groups = 1
max_copies_per_message_id = 2
```

The scanner consumer offset must be absent before publication, after the first scan, and after the second scan. Runtime execution remains pending.

## Count-only privacy boundary

During a scan, physical header UUIDs and exact bytes exist only long enough to create in-memory duplicate observations. Exact bytes are reduced to a domain-separated digest inside the classifier.

The returned result exposes only counts. It does not expose broker addresses, stream/topic/partition/offset, UUIDs, payloads, payload digests, credentials, or raw Iggy errors.

The runtime harness also does not print or retain those values. A future retained packet must preserve the same boundary.

## Profiles authorization remains unchanged

No profile visibility, relationship, block, mute, follow, friendship, audience, ownership, or presentation decision may depend on:

- whether a DLQ scan or runtime harness was performed;
- selected partitions or offsets;
- physical duplicate counts;
- conflicting-payload counts;
- scanner or harness errors;
- broker deduplication configuration;
- consumer offset presence;
- future retained evidence metadata.

Profiles continues to consume authorized owner-port results. The scanner and harness are operational observability for downstream neutralization only.

## Operator interpretation

A bounded scan is not automatically a complete historical inventory. Its meaning depends on the explicitly selected stream, partitions, offsets, retention window, and message cap.

Aggregate receipt health and aggregate physical duplicate health remain independent identifier-free views. They may be compared as operational trends, but they cannot be joined message by message and must not become authorization evidence.

Any `conflicting_payload_groups > 0` result requires manual forensic escalation. The scanner does not identify the affected UUID or provide a destructive action.

## Remaining work

1. execute the source-complete runtime case against a reviewed dedup-disabled disposable service;
2. retain only count-level runtime and absent-offset evidence;
3. define alert thresholds outside Profiles and outside the scanner;
4. design acknowledgement/delete/replay as a separate explicitly authorized workflow;
5. preserve the identifier-free boundary when comparing receipt and physical duplicate trends.

No tests, Cargo commands, formatters, source verifiers, external-Iggy scans, or retained capture were run by the implementation agent.
