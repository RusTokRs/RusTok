# Bounded physical DLQ duplicate rolling window

Status: **transport-neutral rolling state plus moving scanner and server integration source-complete; external runtime evidence pending**.

## Purpose

A fixed explicit-offset scan can classify copies inside one result but cannot relate one copy observed near the end of one advancing cycle with another copy in a later cycle.

`DlqDuplicateRollingWindow` retains opaque observations across complete scan cycles. While copies remain inside the retained window, the existing count-only classifier detects ordinary duplicates and conflicting payloads across cycle boundaries.

The transport-neutral state itself does not move a broker cursor, connect to Iggy, poll messages, store offsets, persist itself, mutate receipts, or authorize Profiles.

## Explicit bounds

The caller supplies:

```text
max_cycles
max_observations_per_cycle
```

Both are positive. Cycle count is capped at 128, and the checked product cannot exceed 10,000 observations. No production default is defined.

## Complete-cycle semantics

Each `push_cycle` supplies one complete scan cycle of opaque observations.

- empty successful cycles are valid;
- oversized cycles fail before mutation;
- candidate classification succeeds before current state is replaced;
- the oldest complete cycle is evicted at capacity;
- partial-cycle eviction is forbidden.

Keeping complete scan cycles makes the loss boundary explicit. It does not make the retained result complete history.

## Identifier-free snapshot

`DlqDuplicateRollingWindowSnapshot` exposes only:

```text
summary
retained_cycles
retained_observations
evicted_cycles
history_truncated
```

The snapshot excludes UUIDs, payloads/digests, partitions, offsets, endpoints, credentials, receipts, timestamps, and raw errors.

## Truncation boundary

After the first complete-cycle eviction:

```text
history_truncated = true
```

An evicted old copy can remove a previously visible relationship. A truncated result describes only the retained bounded window and is not complete history, current-tail proof, or production retention evidence.

## Moving scanner and server integration

The moving scanner and server integration are source-complete.

`IggyDlqDuplicateMovingWindowState` owns independent process-local per-partition cursors. `IggyDlqDuplicateMovingWindowScanner` collects every selected partition into a temporary equal-budget candidate and feeds exactly one complete combined cycle into the rolling state.

The server exposes this path only through explicit:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=moving_window
```

Moving configuration requires reviewed initial offset, per-partition cap, batch size, maximum cycles, and maximum observations per cycle. No moving default is provided. The rolling per-cycle capacity must cover the full fair-cycle budget.

A failed moving cycle preserves connected process-local cursor and rolling state. A new process or replacement connection starts again at the reviewed initial offset with empty rolling history because persistence is deliberately absent.

The server reduces each successful moving snapshot to the existing `DlqDuplicateSummary`; partition IDs, cursor values, and observations remain private.

## Source verification

```bash
node scripts/verify/verify-iggy-dlq-duplicate-rolling-window.mjs
node scripts/verify/verify-iggy-dlq-duplicate-moving-window-scan.mjs
node scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs
```

Tests, Cargo commands, formatters, verifiers, broker scans, server startup, and retained capture were not run while authoring these source slices.

## Remaining work

1. retain a real external-Iggy duplicate split across advancing cycles;
2. review initial offset and acceptable reset frequency per deployment;
3. add persistent cursor ownership only if restart continuity is required;
4. retain server observer execution evidence;
5. define identifier-free telemetry and optional health.
