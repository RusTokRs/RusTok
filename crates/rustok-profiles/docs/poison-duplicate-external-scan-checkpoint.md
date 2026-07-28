# Profiles checkpoint: bounded external DLQ duplicate scan

Status: **global scanner harness and fair-window server source complete; execution and retained evidence pending**.

## What changed

`rustok-iggy` owns a bounded read-only adapter that feeds physical external-Iggy `dlq` messages into the count-only duplicate classifier.

Public API:

```text
IggyDlqDuplicateScanRequest
IggyDlqDuplicateScanWindowPolicy
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

## Global request

The compatibility request is limited to:

- at most 128 unique positive partitions;
- at most 10,000 physical messages globally;
- batches of at most 1,000 messages;
- one explicit start offset applied independently to each partition.

The ordered global budget may be consumed before later partitions are polled.

## Fair snapshot window

The fair policy assigns one equal positive message cap to every selected partition and checks:

```text
partition_count * per_partition_messages <= 10000
batch_size <= per_partition_messages
```

A successful scan attempts every configured partition and combines all observations before classification. This preserves duplicate and conflicting-payload groups that span partitions.

The fair policy remains one fixed window. It does not provide moving cursors, stored progress, cross-cycle duplicate accumulation, current-tail coverage, or complete-history proof.

## Server integration

The event-delivery observer keeps `global_budget` as the default and supports explicit:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=fair_window
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_PER_PARTITION_MESSAGES=<positive cap>
```

This applies only to `outbox_iggy`. `memory` and `outbox_local` remain intentional not-applicable modes and do not require Iggy.

## Source-complete runtime case

The existing opt-in runtime case remains global-budget evidence. It requires one reviewed dedup-disabled disposable external service and publishes four physical messages through production `IggyTransport::move_to_dlq`:

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

A separate multi-partition fair-window harness and retained packet remain pending.

## Count-only privacy boundary

During a scan, physical header UUIDs and exact bytes exist only long enough to create in-memory duplicate observations. Exact bytes are reduced to a domain-separated digest inside the classifier.

The returned result exposes only counts. It does not expose broker addresses, stream/topic/partition/offset, UUIDs, payloads, payload digests, credentials, or raw Iggy errors.

## Profiles authorization remains unchanged

No profile visibility, relationship, block, mute, follow, friendship, audience, ownership, or presentation decision may depend on:

- whether a global or fair DLQ scan was performed;
- selected partitions, offsets, or scan mode;
- physical duplicate or conflicting-payload counts;
- scanner, observer, or harness errors;
- broker deduplication configuration;
- consumer offset presence;
- future retained evidence metadata.

Profiles continues to consume authorized owner-port results. The scanner and observer are operational observability for downstream neutralization only.

## Operator interpretation

A bounded scan is not automatically a complete historical inventory. Its meaning depends on the explicitly selected stream, partitions, offsets, retention window, and message caps.

Aggregate receipt health and aggregate physical duplicate health remain independent identifier-free views. They may be compared as operational trends, but they cannot be joined message by message and must not become authorization evidence.

Any `conflicting_payload_groups > 0` result requires manual forensic escalation. The scanner does not identify the affected UUID or provide a destructive action.

## Remaining work

1. execute and retain the compatibility-global runtime case;
2. add and execute multi-partition fair-window evidence;
3. design moving windows plus bounded cross-cycle duplicate state, or keep fixed windows;
4. design acknowledgement/delete/replay as a separate explicitly authorized workflow;
5. preserve the identifier-free boundary when comparing receipt and physical duplicate trends.

No tests, Cargo commands, formatters, source verifiers, external-Iggy scans, or retained capture were run by the implementation agent.
