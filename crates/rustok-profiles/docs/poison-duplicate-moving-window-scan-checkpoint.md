# Profiles checkpoint: moving physical DLQ duplicate window scan

Status: **Iggy moving scanner, independent process-local cursors, complete-cycle rolling integration, and explicit restart reset source-complete; server composition and runtime evidence pending**.

## Why this matters for Profiles

Profiles still never authorizes from broker, receipt, offset, duplicate, or evidence state. This source slice only improves operational detection when two physical DLQ copies appear in different advancing scan cycles.

The Iggy owner now has a bounded opt-in path:

```text
private per-partition next offsets
  -> one complete fair read-only cycle
  -> bounded cross-cycle rolling classification
  -> identifier-free snapshot
```

No delivery identity, payload, digest, partition, or offset crosses into Profiles.

## Owner paths

```text
scanner/state:
  crates/rustok-iggy/src/dlq_duplicate_moving_window_scan.rs
rolling classifier:
  crates/rustok-iggy/src/dlq_duplicate_rolling_window.rs
machine contract:
  crates/rustok-iggy/contracts/evidence/
    dlq-duplicate-moving-window-scan-source.json
verifier:
  scripts/verify/verify-iggy-dlq-duplicate-moving-window-scan.mjs
owner guide:
  crates/rustok-iggy/docs/dlq-duplicate-moving-window-scan.md
```

## Independent cursor boundary

Each selected partition owns one private in-memory next offset. A successful empty partition keeps its offset unchanged; a partition with messages advances to the checked last offset plus one.

The complete set advances atomically only after every partition succeeds and the combined rolling cycle is accepted. Any incomplete or invalid cycle preserves every cursor and the previous rolling snapshot.

The public snapshot exposes only partition count and how many partitions advanced, never their identities or cursor values.

## Restart choice

This slice intentionally chooses explicit reset rather than implied persistence:

- state is process-local;
- new construction starts every cursor at one reviewed initial offset;
- rolling history starts empty;
- `reset_to_initial_offset()` performs the same operation and increments a count-only generation;
- no restart-safe progress or current-tail claim is made.

A persistent cursor store remains separate work and should be added only when an operator requirement justifies its ownership, fencing, migration, and recovery semantics.

## Profiles authorization boundary

Profiles never authorizes, filters, ranks, hides, reveals, retries, or mutates from:

- moving-window applicability or availability;
- private cursor position or advancement;
- reset generation;
- rolling retention or `history_truncated`;
- physical duplicate or identity-conflict counts;
- scan failure or stable error code;
- server mode, broker configuration, or retained evidence.

Presentation continues to consume authoritative owner-port policy results only.

## Remaining work

1. compose the moving scanner as an explicit opt-in `outbox_iggy` server observer mode;
2. define reviewed configuration and fail-closed startup validation;
3. retain real external-Iggy cross-cycle evidence;
4. decide separately whether restart continuity merits a persistent cursor owner;
5. add identifier-free telemetry and optional operational health.
