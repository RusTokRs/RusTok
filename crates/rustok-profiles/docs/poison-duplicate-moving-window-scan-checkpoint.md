# Profiles checkpoint: moving physical DLQ duplicate window scan

Status: **Iggy moving scanner, independent process-local cursors, complete-cycle rolling integration, explicit reset, and server observer mode source-complete; external runtime evidence pending**.

## Why this matters for Profiles

Profiles still never authorizes from broker, receipt, offset, duplicate, alert, or evidence state. This boundary only improves operational detection when physical DLQ copies appear in different advancing scan cycles.

```text
private per-partition next offsets
  -> one complete fair read-only cycle
  -> bounded cross-cycle rolling classification
  -> count-only alert summary
  -> identifier-free server snapshot
```

No delivery identity, payload, digest, partition, or offset crosses into Profiles.

## Owner paths

```text
scanner/state:
  crates/rustok-iggy/src/dlq_duplicate_moving_window_scan.rs
Iggy observer composition:
  crates/rustok-iggy/src/dlq_duplicate_alert_observer.rs
server composition:
  apps/server/src/services/event_dlq_duplicate_alert_observer.rs
machine contracts:
  crates/rustok-iggy/contracts/evidence/
    dlq-duplicate-moving-window-scan-source.json
    dlq-duplicate-alert-server-observer-source.json
verifiers:
  scripts/verify/verify-iggy-dlq-duplicate-moving-window-scan.mjs
  scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs
```

## Independent cursor boundary

Each selected partition owns one private in-memory next offset. A successful empty partition keeps its offset unchanged; a partition with messages advances to checked last offset plus one.

The complete set advances atomically only after every partition succeeds and the combined rolling cycle is accepted. Any incomplete or invalid cycle preserves every private cursor and the prior rolling snapshot.

The public moving snapshot exposes only partition count and how many partitions advanced, never their identities or cursor values.

## Server observer mode

The server observer mode is source-complete as explicit opt-in:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=moving_window
```

Moving mode requires reviewed fail-closed values for initial offset, per-partition message cap, batch size, rolling maximum cycles, and rolling maximum observations per cycle. There are no moving production defaults.

The server validates configuration before spawning the observer task. Moving scan results are reduced to the existing `DlqDuplicateSummary`, so the alert policy and latest-value runtime remain count-only.

A failed moving cycle publishes unavailable state but preserves the connected observer's process-local cursor and rolling state for the next attempt. Fixed modes remain reconnectable snapshots.

## Restart choice

This boundary keeps explicit reset rather than implied persistence:

- state is process-local;
- new construction or replacement connection starts every cursor at one reviewed initial offset;
- rolling history starts empty;
- `reset_to_initial_offset()` performs the same transition in the library state;
- no restart-safe progress or current-tail claim is made.

A persistent cursor store remains separate work and should be added only when an operator requirement justifies ownership, fencing, migration, and recovery semantics. Deployment review must choose initial offset and acceptable reset frequency.

## Profiles authorization boundary

Profiles never authorizes, filters, ranks, hides, reveals, retries, or mutates from:

- moving-window applicability or availability;
- private cursor position or advancement;
- reset or replacement frequency;
- rolling retention or `history_truncated`;
- physical duplicate or identity-conflict counts;
- scan failure or stable error code;
- server mode, broker configuration, or retained evidence.

Presentation continues to consume authoritative owner-port policy results only.

## Remaining work

1. retain real external-Iggy cross-cycle evidence;
2. review deployment initial offset and reset frequency;
3. decide separately whether restart continuity merits persistent cursor ownership;
4. retain server observer execution evidence;
5. add identifier-free telemetry and optional operational health.

Tests, Cargo commands, formatters, verifiers, server startup, external-Iggy scans, telemetry registration, and retained capture were not run by the implementation agent.
