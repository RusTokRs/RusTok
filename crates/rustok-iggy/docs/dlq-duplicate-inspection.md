# Count-only physical DLQ duplicate inspection

Status: **classifier, bounded scanner, runtime harness, retained tooling, alert policy, latest-value runtime, and mode-aware server observer source-complete; runtime execution and telemetry/health projection pending**.

## Purpose

The neutral poison receipt store answers whether a source delivery is reserved, publishing, published, or acknowledged. It does not answer how many physical copies currently exist in the physical `dlq` topic.

Physical copies can exceed one when broker-side deterministic message-ID deduplication is disabled, expired, evicted, or not preserved by an unsupported path.

The implementation is deliberately split:

```text
physical observations
  -> DlqDuplicateSummary
  -> explicit alert policy
  -> latest-value runtime
  -> mode-aware server observer
  -> future telemetry/health or notification owners
```

## Input and identity boundary

Each observation accepts a non-nil deterministic Iggy header UUID and exact physical payload bytes. Bytes are immediately reduced to a domain-separated SHA-256 value in memory and are never exposed by the observation or summary.

A nil UUID fails closed with:

```text
iggy.dlq_duplicate.identity_invalid
```

## Count-only result

`DlqDuplicateSummary` exposes only:

```text
total_messages
unique_message_ids
duplicate_messages
duplicate_groups
conflicting_payload_groups
max_copies_per_message_id
```

```text
duplicate_messages = total_messages - unique_message_ids
```

An ordinary duplicate has the same deterministic ID and same exact bytes. Reuse of one ID with different exact bytes is an identity conflict and requires manual review.

The summary excludes broker coordinates, UUIDs, payloads/digests, receipt identities, error classification, publisher identity, timestamps, and credentials.

## Mutation boundary

The classifier and scanner cannot acknowledge, store offsets, delete, purge, replay, retry, publish, repair broker state, mutate poison receipts, or choose operator policy.

The alert policy cannot scan, route notifications, persist thresholds, or mutate state. The latest-value runtime cannot start workers, register telemetry/health, deliver notifications, or mutate broker/receipt/Profile state.

## Production identity relationship

`ConsumedContractDecodeFailure::delivery_id` derives one UUID from immutable source coordinates and exact raw bytes. Failure kind, retry count, process identity, time, and random values are excluded.

`to_dlq_entry` attaches that UUID as the Iggy broker message ID. `IggyTransport::move_to_dlq` uses the deterministic publisher path when this ID is present.

`ConsumerPoisonReceiptInspector` remains an independent count-only PostgreSQL view. Receipt and physical duplicate summaries contain no identifiers and cannot be joined message by message.

## Bounded Iggy scanner

`IggyDlqDuplicateScanner` polls only:

```text
topic = dlq
standalone consumer
explicit positive partition
PollingStrategy::offset(explicit_offset)
auto_commit = false
```

One request permits at most 128 partitions, 10,000 messages globally, and batches of 1,000. It validates returned partition, count, monotonic offsets, and header identity before returning only `DlqDuplicateSummary`.

## Count-only alert policy

`DlqDuplicateAlertPolicy` requires explicit warning and critical thresholds for duplicate messages, duplicate groups, and maximum copies for one message ID. It defines no production defaults.

```text
identity conflict -> Critical
critical numeric threshold -> Critical
warning numeric threshold -> Warning
physical duplicate below warning -> Notice
no duplicate -> Clear
```

The evaluation exposes only level and boolean reason flags.

## Latest-value alert runtime

`DlqDuplicateAlertRuntimePublisher` accepts an already-observed summary and a prevalidated policy.

Initial state is generation `0`, unavailable, with no evaluation. Successful and unavailable transitions advance generation through checked arithmetic. `mark_unavailable()` clears the previous evaluation so stale severity is not shown as current.

The runtime is a latest-value channel, not an event log. It adds no serialization or persistence.

## Mode-aware server observer

The host now owns an explicit capability gate across all event-delivery profiles:

```text
memory        -> NotApplicableMemory
outbox_local  -> NotApplicableOutboxLocal
outbox_iggy   -> IggyBundled or IggyExternal
```

For `memory` and `outbox_local`, no Iggy transport is requested, no broker connection is opened, and thresholds are not required. Not-applicable state is valid operation rather than a degraded condition.

For `outbox_iggy`, the observer reuses the exact active transport configuration and opens a separate read-only SDK client:

- bundled mode connects to the existing validated loopback broker;
- external mode uses the reviewed address list;
- missing active Iggy mode fails closed;
- no second transport or broker process is created;
- connection/scan failure publishes unavailable state and retries later;
- event delivery and module projection remain active.

The observer supports all configured domain partitions and delegates every scan to the bounded `auto_commit=false` scanner.

## Safe operational sequence

1. resolve the active event delivery profile;
2. return not-applicable before Iggy access for `memory` or `outbox_local`;
3. for `outbox_iggy`, use the already-active bundled or external configuration;
4. scan bounded explicit offsets with `auto_commit=false`;
5. evaluate through explicit thresholds;
6. publish only the identifier-free latest snapshot;
7. mark unavailable after failures or shutdown;
8. keep telemetry, health, notification delivery, and destructive actions in separate owners.

## Runtime and retained evidence status

The opt-in external harness and privacy-safe retained tooling are source-complete. The canonical execution JSON remains absent until a maintainer runs the reviewed external-Iggy scenario successfully.

The mode-aware server observer is source-complete. Runtime execution, identifier-free telemetry, optional operational health without readiness coupling, and retained server evidence remain pending.

## Source verification

```bash
node scripts/verify/verify-iggy-dlq-duplicate-inspection.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan-runtime.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan-retained.mjs
node scripts/verify/verify-iggy-dlq-duplicate-alert-policy.mjs
node scripts/verify/verify-iggy-dlq-duplicate-alert-runtime.mjs
node scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs
```

No tests, Cargo commands, formatters, verifiers, broker scans, server observers, telemetry registration, alert dispatch, or retained capture were run while defining these source slices.

## Remaining work

1. execute and retain the reviewed external-Iggy duplicate scan packet;
2. define identifier-free telemetry and optional operational health;
3. retain mode-aware server observer execution evidence;
4. define alert routing, cooldown, and suppression separately;
5. design acknowledgement/delete/replay as a separately authorized operation;
6. correlate aggregate receipt and duplicate health without exporting message identities.
