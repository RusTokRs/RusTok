# Count-only physical DLQ duplicate inspection

Status: **classifier, fixed scanners, bounded rolling state, moving scanner integration, runtime harnesses, retained tooling, alert policy, latest-value runtime, and fixed-window mode-aware observer source-complete; moving server composition, runtime execution, and telemetry/health projection pending**.

## Purpose

The neutral poison receipt store answers whether a source delivery is reserved, publishing, published, or acknowledged. It does not answer how many physical copies currently exist in the physical `dlq` topic.

Physical copies can exceed one when broker-side deterministic message-ID deduplication is disabled, expired, evicted, or not preserved by an unsupported path.

```text
physical observations
  -> one-cycle DlqDuplicateSummary
  -> optional bounded cross-cycle rolling state
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

The classifier, rolling state, fixed scanners, and moving scanner cannot acknowledge, store consumer offsets, delete, purge, replay, retry, publish, repair broker state, mutate poison receipts, or choose operator policy.

The alert policy cannot scan, route notifications, persist thresholds, or mutate state. The latest-value runtime cannot start workers, register telemetry/health, deliver notifications, or mutate broker/receipt/Profile state.

## Production identity relationship

`ConsumedContractDecodeFailure::delivery_id` derives one UUID from immutable source coordinates and exact raw bytes. Failure kind, retry count, process identity, time, and random values are excluded.

`to_dlq_entry` attaches that UUID as the Iggy broker message ID. `IggyTransport::move_to_dlq` uses the deterministic publisher path when this ID is present.

`ConsumerPoisonReceiptInspector` remains an independent count-only PostgreSQL view. Receipt and physical duplicate summaries contain no identifiers and cannot be joined message by message.

## Fixed bounded Iggy scanners

`IggyDlqDuplicateScanner` polls only:

```text
topic = dlq
standalone consumer
explicit positive partition
PollingStrategy::offset(explicit_offset)
auto_commit = false
```

One request permits at most 128 partition identifiers, 10,000 messages globally, and batches of 1,000. It validates returned partition, count, monotonic offsets, and header identity before returning only `DlqDuplicateSummary`.

The global message budget is shared across the ordered partition allowlist and can starve later partitions. The opt-in fair policy gives each selected partition one equal cap under the same checked 10,000-message total. Both fixed policies reuse configured explicit offsets and do not retain cross-cycle identity state.

## Bounded cross-cycle rolling state

`DlqDuplicateRollingWindow` retains opaque observations across complete scan cycles so ordinary duplicates or conflicting payloads split across adjacent retained cycles can still be classified together.

The caller supplies positive `max_cycles` and `max_observations_per_cycle`. Cycle count is capped at 128 and their checked product cannot exceed 10,000 observations. No production default is defined.

An oversized cycle fails transactionally. At capacity, the oldest complete cycle is evicted; partial-cycle eviction is forbidden. After the first eviction every identifier-free snapshot reports:

```text
history_truncated = true
```

An evicted older copy can remove a previously visible duplicate relationship. The retained result is therefore not complete history, current-tail proof, or production retention evidence.

## Moving scanner integration

The moving scanner integration is source-complete in `dlq_duplicate_moving_window_scan.rs`.

`IggyDlqDuplicateMovingWindowState` owns independent process-local per-partition cursors. `IggyDlqDuplicateMovingWindowScanner` applies one equal bounded budget per selected partition with explicit offsets and `auto_commit=false`.

Complete-cycle atomicity is mandatory:

1. every selected partition is polled into a temporary candidate;
2. returned partition, count, offsets, UUID, and bounds are validated;
3. the combined opaque observations are pushed as one complete rolling cycle;
4. every private cursor is replaced together only after rolling acceptance.

Any polling, response, offset, classification, or rolling error preserves all cursor and rolling state. Empty partitions are valid and keep their cursor unchanged. Public snapshots retain only rolling counts, partition count, advanced-partition count, and reset generation; no partition ID, cursor value, message identity, payload, or digest is exported.

Progress persistence is deliberately absent. New process-local state starts at one reviewed initial offset. An explicit restart reset through `reset_to_initial_offset()` rewinds all cursors and clears rolling history. No restart-safe progress, current-tail, or complete-history claim is made. A persistent cursor owner remains separate work only if restart continuity is required.

## Count-only alert policy and latest-value runtime

`DlqDuplicateAlertPolicy` requires explicit warning and critical thresholds for duplicate messages, duplicate groups, and maximum copies for one message ID. It defines no production defaults.

```text
identity conflict -> Critical
critical numeric threshold -> Critical
warning numeric threshold -> Warning
physical duplicate below warning -> Notice
no duplicate -> Clear
```

The evaluation exposes only level and boolean reason flags.

`DlqDuplicateAlertRuntimePublisher` accepts an already-observed summary and a prevalidated policy. Initial state is generation `0`, unavailable, with no evaluation. Successful and unavailable transitions advance generation through checked arithmetic. `mark_unavailable()` clears the previous evaluation so stale severity is not shown as current.

The runtime is a latest-value channel, not an event log. It adds no serialization or persistence.

## Mode-aware server observer

The current host observer owns an explicit capability gate:

```text
disabled      -> Disabled
startup issue -> Unavailable, no task or snapshot
memory        -> NotApplicableMemory
outbox_local  -> NotApplicableOutboxLocal
outbox_iggy   -> IggyBundled or IggyExternal
```

For `memory` and `outbox_local`, no Iggy transport is requested and no broker connection is opened. For `outbox_iggy`, the observer reuses the active bundled or external transport configuration and opens only a separate read-only SDK client. Startup and scan failures are non-fatal to event delivery and Profiles.

The current observer still runs fixed global or fair snapshots. Moving-window mode remains a pending explicit opt-in; this PR does not silently change server behavior or configuration defaults.

## Safe operational sequence

1. resolve the active event delivery profile;
2. return not-applicable before Iggy access for `memory` or `outbox_local`;
3. for `outbox_iggy`, reuse the active bundled or external configuration;
4. select one explicitly configured fixed or future moving scan mode;
5. poll with explicit offsets and `auto_commit=false`;
6. for moving mode, atomically retain one complete all-partition cycle;
7. evaluate through explicit thresholds;
8. publish only the identifier-free latest snapshot;
9. mark unavailable after startup, connection, scan, or shutdown failures without stopping the host;
10. keep persistent cursor ownership, telemetry, health, notification delivery, and destructive actions separate.

## Runtime and retained evidence status

The fixed external harnesses and privacy-safe retained tooling are source-complete. Canonical execution JSON remains absent until maintainers run reviewed external-Iggy scenarios.

The rolling state and moving scanner integration are source-complete as library components. Moving server composition and reviewed configuration, cross-cycle external-Iggy execution, optional persistent cursor ownership, telemetry, operational health, and retained server evidence remain pending.

## Source verification

```bash
node scripts/verify/verify-iggy-dlq-duplicate-inspection.mjs
node scripts/verify/verify-iggy-dlq-duplicate-rolling-window.mjs
node scripts/verify/verify-iggy-dlq-duplicate-moving-window-scan.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan-runtime.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan-retained.mjs
node scripts/verify/verify-iggy-dlq-duplicate-alert-policy.mjs
node scripts/verify/verify-iggy-dlq-duplicate-alert-runtime.mjs
node scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs
```

No tests, Cargo commands, formatters, verifiers, broker scans, server observers, telemetry registration, alert dispatch, or retained capture were run while defining these source slices.

## Remaining work

1. execute and retain the reviewed fixed external-Iggy duplicate scan packets;
2. compose moving-window scanning as an explicit mode-aware server opt-in;
3. define reviewed moving-window configuration and fail-closed validation;
4. retain a real external-Iggy duplicate split across advancing cycles;
5. add a persistent cursor owner only if restart continuity is required;
6. define identifier-free telemetry and optional operational health;
7. retain server observer execution evidence;
8. define alert routing, cooldown, and suppression separately;
9. design acknowledgement/delete/replay as a separately authorized operation;
10. correlate aggregate receipt and duplicate health without exporting message identities.
