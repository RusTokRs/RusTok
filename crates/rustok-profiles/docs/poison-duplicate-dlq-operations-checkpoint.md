# Profiles checkpoint: physical DLQ duplicate operations

Status: **count-only classifier, fixed scanners, rolling state, moving scanner, and moving-window server composition source-complete; runtime evidence pending**.

## Why this matters for Profiles

Profiles authorization remains independent of broker and receipt state. Operational tooling must still distinguish unique physical copies, ordinary duplicates, conflicting bytes for one deterministic ID, and copies split across advancing cycles.

```text
physical DLQ scan
  -> count-only duplicate summary
  -> explicit alert policy
  -> identifier-free latest snapshot
```

## Delivered boundaries

```text
classifier:       crates/rustok-iggy/src/dlq_duplicate_inspection.rs
fixed scanner:    crates/rustok-iggy/src/dlq_duplicate_external_scan.rs
rolling state:    crates/rustok-iggy/src/dlq_duplicate_rolling_window.rs
moving scanner:   crates/rustok-iggy/src/dlq_duplicate_moving_window_scan.rs
Iggy observer:    crates/rustok-iggy/src/dlq_duplicate_alert_observer.rs
server observer:  apps/server/src/services/event_dlq_duplicate_alert_observer.rs
```

The moving-window server composition is source-complete. `global_budget` remains default; `moving_window` is explicit opt-in with reviewed fail-closed configuration.

## Moving semantics

Each selected partition owns one private cursor in process memory. A complete all-partition candidate is collected before mutation. Every cursor and rolling history update atomically only after acceptance.

Failed moving cycles preserve connected private cursor and rolling state. A new process or replacement connection starts from the reviewed initial offset with empty rolling history. No restart-safe progress, current-tail, complete-history, or exactly-once claim is made.

The server publishes only the existing count-only alert evaluation. It does not expose partition identity, cursor values, offsets, message IDs, payloads/digests, observations, credentials, raw errors, thresholds, or source counts.

## Profiles authorization boundary

No profile visibility, ownership, follower access, relationship, block, mute, audience, storefront presentation, author card, ranking, retry, or mutation may depend on:

- observer startup, availability, or generation;
- fixed or moving scan selection;
- private cursor position or reset;
- rolling retention or `history_truncated`;
- duplicate counts, conflicts, or alert level;
- Iggy deployment or retained evidence.

Profiles presentation continues to consume authoritative owner-port policy results only.

## Remaining work

1. retain fixed and moving external-Iggy runtime evidence;
2. review initial offset and reset frequency per deployment;
3. add persistent cursor ownership only if restart continuity is required;
4. add identifier-free telemetry and optional health;
5. retain server observer execution evidence;
6. keep notification delivery and destructive reconciliation separate.

Tests, Cargo commands, formatters, verifiers, server startup, external-Iggy scans, alert delivery, telemetry, and retained capture were not run by the implementation agent.
