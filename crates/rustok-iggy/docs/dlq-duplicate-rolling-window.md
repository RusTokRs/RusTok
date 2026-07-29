# Bounded physical DLQ duplicate rolling window

Status: **transport-neutral rolling-window state source-complete; scanner, cursor, persistence, server integration, and runtime evidence pending**.

## Purpose

A fixed explicit-offset scan can classify duplicates that appear inside one scan result. It cannot relate one copy observed near the end of one cycle with another copy observed in a later cycle after the scanner advances.

`DlqDuplicateRollingWindow` retains opaque duplicate observations across a bounded number of complete scan cycles. While both copies remain inside the retained window, the existing `DlqDuplicateSummary` classifier detects ordinary duplicates and conflicting payloads across cycle boundaries.

The state does not move a broker cursor, connect to Iggy, poll messages, store offsets, persist itself, mutate receipts, or authorize Profiles.

## Explicit bounds

The caller constructs `DlqDuplicateRollingWindowPolicy` with:

```text
max_cycles
max_observations_per_cycle
```

Both values must be positive. `max_cycles` is capped at 128, and the checked product must satisfy:

```text
max_cycles * max_observations_per_cycle <= 10000
```

The crate defines no production default for cadence, cycle count, observation count, or retention duration.

## Complete-cycle semantics

Each `push_cycle` call supplies one complete scan cycle of opaque `DlqDuplicateObservation` values.

- empty successful cycles are valid;
- an oversized cycle fails before state mutation;
- the candidate window is classified before it replaces current state;
- when cycle capacity is full, the oldest complete cycle is evicted;
- partial-cycle eviction is not allowed.

Keeping complete cycles makes the loss boundary explicit. It does not make the retained observations a complete history.

## Identifier-free snapshot

`DlqDuplicateRollingWindowSnapshot` exposes only:

```text
summary
retained_cycles
retained_observations
evicted_cycles
history_truncated
```

`summary` remains the existing count-only `DlqDuplicateSummary`. The snapshot exposes no UUID, payload digest, partition, offset, stream, endpoint, credential, receipt identity, timestamp, or raw error.

## Cross-cycle classification

The window combines all retained observations before calling `summarize_dlq_duplicates`.

```text
cycle 1: A1
cycle 2: A2, same deterministic ID and exact bytes
result: ordinary physical duplicate
```

```text
cycle 1: B1
cycle 2: B2, same deterministic ID and different exact bytes
result: identity conflict requiring manual review
```

This relationship is preserved only while all relevant copies remain retained.

## Truncation boundary

After the first complete-cycle eviction:

```text
history_truncated = true
evicted_cycles > 0
```

The flag never returns to false for that in-memory state. An identity relationship can disappear when its older copy is evicted. Therefore a truncated snapshot describes only the currently retained bounded window and is not complete history, current-tail proof, or production retention evidence.

## Source contract and verification

Machine contract:

```text
crates/rustok-iggy/contracts/evidence/
  dlq-duplicate-rolling-window-source.json
```

Static verifier:

```bash
node scripts/verify/verify-iggy-dlq-duplicate-rolling-window.mjs
```

Focused unit scenarios cover invalid bounds, ordinary duplicates split across cycles, identity conflicts split across cycles, complete-cycle eviction, and transactional oversized-cycle rejection.

## Remaining integration

Scanner integration remains pending. A later reviewed slice must:

1. feed one complete fair per-partition scan cycle into the state without exporting identifiers;
2. define independent per-partition cursor advancement;
3. choose explicit persistence or restart-reset semantics;
4. compose the mode-aware server observer;
5. prove cross-cycle behavior on external Iggy;
6. publish only identifier-free telemetry or health.

No current API claims moving cursors, persisted progress, restart-safe state, current-tail coverage, complete history, production retention sufficiency, or exactly-once delivery.
