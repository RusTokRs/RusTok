# Profiles checkpoint: physical DLQ duplicate operations

Status: **classifier, fixed scanners, bounded rolling state, moving scanner integration, runtime harnesses, retained tooling, alert policy, latest-value runtime, and fixed-window mode-aware server observer source-complete; moving server composition, runtime execution, and telemetry/health projection pending**.

## Why this matters for Profiles

Profiles authorization remains independent of broker and receipt state. Downstream processing must still distinguish:

- one durable neutral result with one physical DLQ copy;
- one durable neutral result with repeated identical physical copies;
- one deterministic DLQ ID associated with conflicting exact bytes;
- copies of the same physical duplicate observed in adjacent retained scan cycles.

The Iggy-owned classifier exposes these distinctions without message identities or payloads. Fixed scanners provide bounded one-cycle snapshots. The rolling state preserves opaque relationships across retained cycles. The moving scanner integration is source-complete and supplies whole fair cycles through private process-local per-partition cursors. Alert/runtime/server projections remain identifier-free. None becomes a Profiles authorization input.

## Owner boundaries

```text
classifier:          crates/rustok-iggy/src/dlq_duplicate_inspection.rs
rolling state:       crates/rustok-iggy/src/dlq_duplicate_rolling_window.rs
fixed scanner:       crates/rustok-iggy/src/dlq_duplicate_external_scan.rs
moving scanner:      crates/rustok-iggy/src/dlq_duplicate_moving_window_scan.rs
policy:              crates/rustok-iggy/src/dlq_duplicate_alert_policy.rs
runtime:             crates/rustok-iggy/src/dlq_duplicate_alert_runtime.rs
Iggy observer:       crates/rustok-iggy/src/dlq_duplicate_alert_observer.rs
server observer:     apps/server/src/services/event_dlq_duplicate_alert_observer.rs
```

Machine contracts:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-inspection-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-rolling-window-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-moving-window-scan-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-policy-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-runtime-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-server-observer-source.json
```

Verifiers:

```text
scripts/verify/verify-iggy-dlq-duplicate-inspection.mjs
scripts/verify/verify-iggy-dlq-duplicate-rolling-window.mjs
scripts/verify/verify-iggy-dlq-duplicate-moving-window-scan.mjs
scripts/verify/verify-iggy-dlq-duplicate-external-scan.mjs
scripts/verify/verify-iggy-dlq-duplicate-alert-policy.mjs
scripts/verify/verify-iggy-dlq-duplicate-alert-runtime.mjs
scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs
```

## Count-only projections

The physical summary exposes only aggregate counts:

```text
total_messages
unique_message_ids
duplicate_messages
duplicate_groups
conflicting_payload_groups
max_copies_per_message_id
```

The rolling snapshot adds only:

```text
retained_cycles
retained_observations
evicted_cycles
history_truncated
```

The moving snapshot adds only:

```text
partition_count
advanced_partitions
reset_generation
```

Policy and runtime continue to expose only level, reason flags, generation, availability, and evaluation. These projections exclude broker endpoints, stream coordinates, partition IDs, cursor values, UUIDs, payloads/digests, receipt identities, producer identities, credentials, timestamps, and raw Iggy errors.

## Duplicate classification

An ordinary physical duplicate requires the same deterministic Iggy header UUID and the same exact bytes. Bytes are compared through an in-memory domain-separated SHA-256 digest that is never exposed.

A repeated UUID with different exact bytes increments `conflicting_payload_groups` and requires manual review. It is not silently collapsed into a normal duplicate.

## Fixed scan boundary

Fixed global and fair scanners borrow an already connected `IggyClient` and use:

```text
topic = dlq
standalone consumer
explicit partition
explicit offset
auto_commit = false
```

Global mode accepts no more than 128 unique positive partitions, 10,000 physical messages total, and batches of 1,000. Fair mode gives every selected partition one equal cap under the same checked 10,000-message total.

The current server observer still uses fixed snapshots and does not own moving progress. Fixed scanners do not use consumer groups, stored-offset `next` polling, offset storage, acknowledgement, topology discovery, publication, delete/purge, replay/retry, receipt mutation, or client shutdown.

## Bounded cross-cycle state

`DlqDuplicateRollingWindowPolicy` requires explicit positive `max_cycles` and `max_observations_per_cycle`. Cycle count is capped at 128 and their checked product cannot exceed 10,000. No production default is provided.

`push_cycle` accepts one complete cycle of opaque observations. It detects ordinary and conflicting duplicates split across retained cycles. Oversized input fails without changing current state.

When cycle capacity is reached, the oldest complete cycle is evicted. Partial-cycle eviction is forbidden. Every later snapshot reports:

```text
history_truncated = true
```

An evicted old copy can remove a relationship from the retained summary. The snapshot represents only the bounded retained window and is not current-tail or complete-history evidence.

## Moving scanner integration

The moving scanner integration is source-complete.

Every selected partition owns one private process-local per-partition cursor. A complete cycle:

1. polls every selected partition with one equal bounded budget;
2. validates partition, count, strictly increasing offsets, UUID, and checked advancement;
3. combines observations privately;
4. pushes one complete rolling cycle;
5. replaces every cursor together only after rolling acceptance.

A failed or incomplete cycle preserves all cursors and rolling state. Empty partitions are successful and keep their cursor unchanged.

Progress persistence is deliberately absent. New state starts from one reviewed initial offset. Explicit restart reset via `reset_to_initial_offset()` rewinds all cursors and clears rolling history while incrementing only a count-only generation. No restart-safe progress, current-tail, or complete-history claim is made.

A persistent cursor owner remains separate work only if restart continuity is required.

## Alert and server boundaries

The caller supplies warning and critical thresholds; the library provides no production defaults. Identity conflicts are always critical. The latest-value runtime clears stale evaluation on unavailable transitions and is not an audit log.

The server handles disabled, startup-unavailable, memory, outbox-local, bundled-Iggy, and external-Iggy modes explicitly. The current observer remains fixed-window. Moving scan is not silently enabled; it requires a separate reviewed server observer mode and configuration surface.

## Relationship to receipt health

The PostgreSQL `ConsumerPoisonReceiptInspector` remains an independent count-only summary of receipt progress. Neither side exports identifiers, so the views cannot be joined message by message.

Aggregate operational interpretation may compare trends, but those interpretations never become Profiles authorization inputs.

## Profiles authorization boundary

No profile visibility, relationship, block, mute, follow, friendship, audience, storefront, GraphQL, author-card, or presentation decision may depend on:

- event delivery profile or Iggy deployment mode;
- observer startup, applicability, availability, or generation;
- partition ordering, fairness, private cursor position, or reset generation;
- rolling retention or eviction state, including `history_truncated`;
- physical DLQ copy or duplicate-group counts;
- identity-conflict presence;
- alert level or threshold flags;
- scanner, runtime, telemetry, health, or alert-delivery state;
- receipt recovery counts;
- deduplication or alert-threshold configuration;
- retained evidence metadata.

Profiles presentation continues to consume authorized owner-port results. These components only improve operational observability of downstream neutralization.

## Remaining work

1. execute and retain the reviewed fixed external-Iggy duplicate scan packets;
2. compose moving scanning as an explicit mode-aware server opt-in;
3. define reviewed moving configuration and fail-closed startup validation;
4. retain real external-Iggy cross-cycle evidence;
5. add a persistent cursor owner only if restart continuity is required;
6. define identifier-free telemetry and optional operational health;
7. retain server observer execution evidence;
8. define alert routing, cooldown, and suppression outside Profiles and policy/runtime;
9. define acknowledgement/delete/replay separately with explicit authorization;
10. keep aggregate receipt and duplicate observations identifier-free.

Tests, Cargo commands, formatters, verifiers, server observers, external-Iggy scans, telemetry registration, alert dispatch, and retained capture were not run by the implementation agent.
