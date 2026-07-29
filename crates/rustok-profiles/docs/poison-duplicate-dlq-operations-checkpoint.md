# Profiles checkpoint: physical DLQ duplicate operations

Status: **classifier, bounded scanner, bounded rolling state, runtime harness, retained tooling, alert policy, latest-value runtime, and mode-aware server observer source-complete; scanner/cursor integration, runtime execution, and telemetry/health projection pending**.

## Why this matters for Profiles

Profiles authorization remains independent of broker and receipt state. Downstream processing must still distinguish:

- one durable neutral result with one physical DLQ copy;
- one durable neutral result with repeated identical physical copies;
- one deterministic DLQ ID associated with conflicting exact bytes;
- copies of the same physical duplicate observed in adjacent retained scan cycles.

The Iggy-owned classifier exposes these distinctions without message identities or payloads. The bounded scanner supplies observations through explicit-offset polling without storing progress. The rolling state can preserve opaque relationships across complete retained cycles. The alert policy evaluates only the count-only summary. The latest-value runtime and server observer publish only identifier-free operational state. None becomes a Profiles authorization input.

## Owner boundaries

```text
classifier:      crates/rustok-iggy/src/dlq_duplicate_inspection.rs
rolling state:   crates/rustok-iggy/src/dlq_duplicate_rolling_window.rs
scanner:         crates/rustok-iggy/src/dlq_duplicate_external_scan.rs
policy:          crates/rustok-iggy/src/dlq_duplicate_alert_policy.rs
runtime:         crates/rustok-iggy/src/dlq_duplicate_alert_runtime.rs
Iggy observer:   crates/rustok-iggy/src/dlq_duplicate_alert_observer.rs
server observer: apps/server/src/services/event_dlq_duplicate_alert_observer.rs
```

Machine contracts:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-inspection-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-rolling-window-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-policy-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-runtime-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-server-observer-source.json
```

Verifiers:

```text
scripts/verify/verify-iggy-dlq-duplicate-inspection.mjs
scripts/verify/verify-iggy-dlq-duplicate-rolling-window.mjs
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

The policy evaluation exposes only:

```text
level
physical_duplicates
identity_conflict
duplicate_messages_threshold_reached
duplicate_groups_threshold_reached
max_copies_threshold_reached
```

The runtime snapshot exposes only:

```text
generation
available
evaluation
```

These projections exclude broker endpoints, stream coordinates, UUIDs, payloads/digests, receipt identities, producer identities, credentials, timestamps, and raw Iggy errors. Policy/runtime projections also exclude source counts and raw threshold values.

## Duplicate classification

An ordinary physical duplicate requires the same deterministic Iggy header UUID and the same exact bytes. Bytes are compared through an in-memory domain-separated SHA-256 digest that is never exposed.

A repeated UUID with different exact bytes increments `conflicting_payload_groups` and requires manual review. It is not silently collapsed into a normal duplicate.

## Bounded scan boundary

The scanner borrows an already connected `IggyClient` and uses:

```text
topic = dlq
standalone consumer
explicit partition
explicit offset
auto_commit = false
```

Global mode accepts no more than 128 unique positive partitions, 10,000 physical messages total, and batches of 1,000. Fair mode gives every selected partition one equal cap under the same checked 10,000-message total.

Each current polling cycle reuses the configured explicit start offset. The observer therefore still measures a repeated bounded snapshot and does not own a moving cursor, tail coverage, or complete-history semantics.

The scanner does not use a consumer group, stored-offset `next` polling, offset storage, acknowledgement, topology discovery, publication, delete/purge, replay/retry, receipt mutation, or client shutdown.

## Bounded cross-cycle state

`DlqDuplicateRollingWindowPolicy` requires explicit positive `max_cycles` and `max_observations_per_cycle`. Cycle count is capped at 128 and their checked product cannot exceed 10,000. No production default is provided.

`push_cycle` accepts one complete cycle of opaque observations. It detects ordinary and conflicting duplicates split across retained cycles. Oversized input fails without changing the current state.

When cycle capacity is reached, the oldest complete cycle is evicted. Partial-cycle eviction is forbidden. Every later snapshot reports:

```text
history_truncated = true
```

An evicted old copy can remove a relationship from the retained summary. The snapshot therefore represents only the bounded retained window and is not current-tail or complete-history evidence.

The state does not connect to Iggy, move or store cursors, persist itself, define restart recovery, compose the server observer, or register telemetry. Those remain separate owners and follow-up evidence.

## Alert policy boundary

The caller supplies warning and critical thresholds for duplicate messages, duplicate groups, and max copies per message ID. The library provides no production defaults.

```text
identity conflict -> Critical
critical numeric threshold -> Critical
warning numeric threshold -> Warning
physical duplicate below warning -> Notice
no duplicate -> Clear
```

The policy does not choose notification routing, paging, cooldown, suppression, scan cadence, threshold persistence, or destructive action.

## Latest-value alert runtime

The runtime accepts an already observed summary and a prevalidated policy:

```text
summary -> policy evaluation -> latest snapshot -> read-only subscribers
```

The publisher is single-writer. Initial state is generation `0`, unavailable, with no evaluation. Every successful or unavailable transition increments generation through checked arithmetic.

Unavailable publication clears the previous evaluation so stale severity does not remain current. The channel retains only the latest state and is not an audit log.

## Mode-aware server observer

The server handles every event-delivery and observer startup mode explicitly:

```text
disabled      -> Disabled
startup issue -> Unavailable, no task or snapshot
memory        -> NotApplicableMemory
outbox_local  -> NotApplicableOutboxLocal
outbox_iggy   -> IggyBundled or IggyExternal
```

For `memory` and `outbox_local`, absence of Iggy is expected platform behavior. For `outbox_iggy`, the observer reuses the active transport configuration and opens only a separate read-only SDK client. Observer-specific startup, connection, scan, and shutdown failures are non-fatal to event delivery and module projection.

The source-complete rolling state is not yet composed into this observer. No second transport, persisted cursor, notification dispatch, or readiness dependency is introduced.

## Runtime and retained status

The external-Iggy harness and retained execution tooling are source-complete. The canonical retained execution packet remains absent until a maintainer performs the reviewed external-Iggy run.

The rolling state is source-complete as a transport-neutral component. Scanner observation feeding, independent per-partition cursor advancement, persistence/restart semantics, cross-cycle external-Iggy evidence, mode-aware composition, telemetry, and optional operational health remain pending.

## Relationship to receipt health

The PostgreSQL `ConsumerPoisonReceiptInspector` remains an independent count-only summary of receipt progress. Neither side exports identifiers, so the views cannot be joined message by message.

Aggregate operational interpretation may compare trends, but those interpretations never become Profiles authorization inputs.

## Profiles authorization boundary

No profile visibility, relationship, block, mute, follow, friendship, audience, storefront, GraphQL, author-card, or presentation decision may depend on:

- event delivery profile or Iggy deployment mode;
- observer startup, applicability, availability, or generation;
- partition ordering, fairness, fixed-window selection, or budget exhaustion;
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

1. execute and retain the reviewed external-Iggy duplicate scan packet;
2. feed complete fair scanner cycles into rolling state without identifier export;
3. define independent per-partition cursor advancement and persistence/restart semantics;
4. prove cross-cycle behavior on external Iggy and compose the mode-aware observer;
5. define identifier-free telemetry and optional operational health without readiness coupling;
6. retain mode-aware server observer execution evidence;
7. define alert routing, cooldown, and suppression outside Profiles and the policy/runtime;
8. define acknowledgement/delete/replay separately with explicit authorization;
9. keep aggregate receipt and duplicate observations identifier-free.

Tests, Cargo commands, formatters, verifiers, server observers, external-Iggy scans, telemetry registration, alert dispatch, and retained capture were not run by the implementation agent.
