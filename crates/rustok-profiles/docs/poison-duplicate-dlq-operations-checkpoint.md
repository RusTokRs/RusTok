# Profiles checkpoint: physical DLQ duplicate operations

Status: **classifier, bounded scanner, runtime harness, retained tooling, alert policy, latest-value runtime, and mode-aware server observer source-complete; runtime execution and telemetry/health projection pending**.

## Why this matters for Profiles

Profiles authorization remains independent of broker and receipt state. Downstream processing must still distinguish:

- one durable neutral result with one physical DLQ copy;
- one durable neutral result with repeated identical physical copies;
- one deterministic DLQ ID associated with conflicting exact bytes.

The Iggy-owned classifier exposes this distinction without message identities or payloads. The bounded scanner supplies observations through explicit-offset polling without storing progress. The alert policy evaluates only the count-only summary. The latest-value runtime and server observer publish only identifier-free operational state. None becomes a Profiles authorization input.

## Owner boundaries

```text
classifier:      crates/rustok-iggy/src/dlq_duplicate_inspection.rs
scanner:         crates/rustok-iggy/src/dlq_duplicate_external_scan.rs
policy:          crates/rustok-iggy/src/dlq_duplicate_alert_policy.rs
runtime:         crates/rustok-iggy/src/dlq_duplicate_alert_runtime.rs
Iggy observer:   crates/rustok-iggy/src/dlq_duplicate_alert_observer.rs
server observer: apps/server/src/services/event_dlq_duplicate_alert_observer.rs
```

Machine contracts:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-inspection-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-policy-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-runtime-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-server-observer-source.json
```

Verifiers:

```text
scripts/verify/verify-iggy-dlq-duplicate-inspection.mjs
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

It accepts no more than 128 unique positive partitions, 10,000 physical messages globally, and batches of 1,000. It validates returned partition/count and monotonic offsets before returning only `DlqDuplicateSummary`.

The scanner does not use a consumer group, stored-offset `next` polling, offset storage, acknowledgement, topology discovery, publication, delete/purge, replay/retry, receipt mutation, or client shutdown.

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

The server handles every event-delivery profile explicitly:

```text
memory        -> NotApplicableMemory
outbox_local  -> NotApplicableOutboxLocal
outbox_iggy   -> IggyBundled or IggyExternal
```

For `memory` and `outbox_local`, absence of Iggy is expected platform behavior:

- no shared `IggyTransport` is requested;
- no broker client is opened;
- no alert thresholds are required;
- not-applicable state is not an error, readiness failure, or Profiles degradation.

For `outbox_iggy`, the observer reuses the exact active transport configuration and opens only a separate read-only SDK client. Bundled mode connects to the existing validated loopback broker; external mode uses the reviewed address list. A missing active Iggy mode fails closed instead of being guessed.

Connection failure, scan failure, and shutdown publish unavailable state while event delivery and module projection remain active. The observer does not create a second transport, start or stop a bundled process, commit offsets, or dispatch notifications.

## Runtime and retained status

The external-Iggy harness and retained execution tooling are source-complete. The harness creates controlled ordinary-duplicate and identity-conflict fixtures through production publication, scans the same explicit offset twice, and requires no stored consumer offset before or after either scan.

The canonical retained execution packet remains absent until a maintainer performs the reviewed external-Iggy run. The mode-aware server observer is source-complete; telemetry, optional operational health, and retained server execution evidence remain pending.

## Relationship to receipt health

The PostgreSQL `ConsumerPoisonReceiptInspector` remains an independent count-only summary of receipt progress. Neither side exports identifiers, so the views cannot be joined message by message.

Aggregate operational interpretation may compare trends, but those interpretations never become Profiles authorization inputs.

## Profiles authorization boundary

No profile visibility, relationship, block, mute, follow, friendship, audience, storefront, GraphQL, author-card, or presentation decision may depend on:

- event delivery profile or Iggy deployment mode;
- observer applicability, availability, or generation;
- physical DLQ copy or duplicate-group counts;
- identity-conflict presence;
- alert level or threshold flags;
- scan partition or offset selection;
- scanner, runtime, telemetry, health, or alert-delivery state;
- receipt recovery counts;
- deduplication or alert-threshold configuration;
- retained evidence metadata.

Profiles presentation continues to consume authorized owner-port results. These components only improve operational observability of downstream neutralization.

## Remaining work

1. execute and retain the reviewed external-Iggy duplicate scan packet;
2. define identifier-free telemetry and optional operational health without readiness coupling;
3. retain mode-aware server observer execution evidence;
4. define alert routing, cooldown, and suppression outside Profiles and the policy/runtime;
5. define acknowledgement/delete/replay separately with explicit authorization;
6. keep aggregate receipt and duplicate observations identifier-free.

Tests, Cargo commands, formatters, verifiers, server observers, external-Iggy scans, telemetry registration, alert dispatch, and retained capture were not run by the implementation agent.
