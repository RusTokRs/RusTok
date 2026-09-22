# Count-only physical DLQ duplicate inspection

Status: **classifier, fixed scanners, bounded rolling state, moving scanner, moving-window server composition, alert policy, and latest-value runtime source-complete; runtime execution pending**.

## Purpose

Neutral poison receipts describe durable recovery progress. They do not describe how many physical copies exist in the transport-wide `dlq` topic. This boundary classifies physical copies without exporting identities or making broker state a Profiles authorization input.

```text
physical observations
  -> DlqDuplicateSummary
  -> optional bounded rolling state
  -> fixed or moving Iggy observer
  -> explicit alert policy
  -> identifier-free latest runtime snapshot
```

## Count-only classification

Each observation accepts a non-nil deterministic Iggy header UUID and exact payload bytes. Bytes are immediately reduced to a domain-separated SHA-256 digest in memory.

`DlqDuplicateSummary` exposes only:

```text
total_messages
unique_message_ids
duplicate_messages
duplicate_groups
conflicting_payload_groups
max_copies_per_message_id
```

An ordinary duplicate has the same deterministic ID and exact bytes. Reuse of one ID with different bytes is an identity conflict requiring manual review.

## Fixed scanners

`global_budget` consumes one shared cap across the ordered partition allowlist. `fair_window` gives every selected partition one equal cap under a checked total of 10,000 messages. Both use explicit offsets and `auto_commit=false`, and both reuse one configured start offset on every poll.

## Bounded rolling and moving state

`DlqDuplicateRollingWindow` retains complete cycles under checked memory bounds and permanently marks `history_truncated` after the first cycle eviction.

`IggyDlqDuplicateMovingWindowState` owns private process-local per-partition cursors. One complete all-partition candidate is collected before mutation. Cursors and rolling history update together only after the combined cycle is accepted.

No cursor or rolling observation is persisted. A new process or replacement connection starts at one reviewed initial offset with empty rolling history.

## Moving-window server observer

The moving-window server composition is source-complete.

The existing observer accepts explicit:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=moving_window
```

`global_budget` remains the default. Moving mode requires reviewed fail-closed values for initial offset, per-partition cap, batch size, rolling maximum cycles, and rolling maximum observations per cycle. The complete fair-cycle budget must fit one rolling cycle.

The Iggy observer stores moving state internally, calls the moving scanner through `summarize(&mut self)`, and reduces a successful moving snapshot to the existing count-only summary before alert evaluation.

A failed moving cycle marks the public runtime unavailable but preserves connected private process-local per-partition cursors and rolling state for the next attempt. Fixed scan failures remain reconnectable snapshots.

## Alert runtime

`DlqDuplicateAlertPolicy` requires six explicit warning/critical thresholds and has no production defaults. Identity conflict is always critical.

`DlqDuplicateAlertRuntimePublisher` retains only the latest identifier-free evaluation. Connection, scan, and shutdown failures mark it unavailable and clear stale evaluation. Event delivery and module projection remain active.

## Privacy and mutation boundary

Snapshots and logs exclude endpoints, credentials, stream/topic/partition/offset, private cursor values, UUIDs, payloads/digests, rolling observations, receipt identities, raw errors, raw thresholds, and source counts.

The classifier, scanners, rolling state, and observer cannot store broker offsets, acknowledge, publish, delete, purge, replay, retry, mutate receipts, start/stop shared transport, or authorize Profiles.

## Runtime evidence status

Source verification paths:

```bash
node scripts/verify/verify-iggy-dlq-duplicate-inspection.mjs
node scripts/verify/verify-iggy-dlq-duplicate-rolling-window.mjs
node scripts/verify/verify-iggy-dlq-duplicate-moving-window-scan.mjs
node scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs
```

Tests, Cargo commands, formatters, verifiers, broker scans, server startup, alert delivery, and retained capture were not run while defining these source slices.

## Remaining work

1. retain fixed and moving external-Iggy execution evidence;
2. review moving initial offset and reset frequency per deployment;
3. add persistent cursor ownership only if restart continuity is required;
4. add identifier-free telemetry and optional health;
5. retain server observer execution evidence;
6. define notification delivery/suppression and destructive reconciliation separately.
