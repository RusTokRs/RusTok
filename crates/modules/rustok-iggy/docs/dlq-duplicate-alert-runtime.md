# Count-only DLQ duplicate alert runtime

Status: **latest-value runtime and mode-aware server observer source-complete; runtime execution pending**.

## Purpose

`DlqDuplicateAlertRuntimePublisher` composes two already separated owner inputs:

```text
DlqDuplicateSummary
DlqDuplicateAlertPolicy
```

It evaluates an already-observed count-only summary and publishes one identifier-free latest-value snapshot for telemetry or health consumers.

The reusable runtime does not scan Iggy, read poison receipts, choose thresholds, dispatch notifications, or perform reconciliation.

## Public API

```text
DlqDuplicateAlertRuntimePublisher
DlqDuplicateAlertRuntimeSubscriber
DlqDuplicateAlertRuntimeSnapshot
DlqDuplicateAlertRuntimeError
```

The API is not feature-gated on the Iggy SDK. It depends only on the transport-neutral duplicate summary, the validated alert policy, and Tokio's in-memory watch channel.

## Single-writer publisher

The publisher owns:

```text
validated alert policy
current generation
tokio watch sender
```

Mutation methods require `&mut self`. This makes the publisher a deliberate single-writer component and prevents concurrent calls from publishing a lower generation after a higher generation.

A runtime host may create any number of read-only subscribers through `subscribe()`.

## Initial snapshot

A newly created channel contains:

```text
generation = 0
available = false
evaluation = None
```

Initial unavailability is explicit. No severity is invented before the first successful observation.

## Successful publication

`publish(&DlqDuplicateSummary)`:

1. advances generation with checked arithmetic;
2. evaluates the summary through the prevalidated policy;
3. creates `available = true` with `evaluation = Some(...)`;
4. replaces the watch channel's latest value;
5. returns the exact published snapshot.

Only the latest snapshot is retained in memory. The runtime is not an event log and does not guarantee that a slow subscriber observes every intermediate generation.

## Unavailable transition

`mark_unavailable()`:

1. advances generation;
2. publishes `available = false`;
3. sets `evaluation = None`;
4. replaces the previous latest value.

Clearing evaluation is required. A stale `Warning` or `Critical` result must not continue to appear current after the observer reports that its source snapshot is unavailable.

## Subscriber boundary

A subscriber can:

```text
current()
changed().await
```

`current()` returns the latest copied snapshot. `changed()` waits for a newer watch value and then returns it.

Subscribers cannot publish, alter generation, change policy, scan the broker, mutate offsets, or modify receipt/Profile state.

When every publisher is dropped, a waiting subscriber fails with:

```text
iggy.dlq_duplicate.alert_runtime_publisher_closed
```

## Snapshot projection

The snapshot contains only:

```text
generation
available
evaluation
```

The optional evaluation remains the identifier-free policy result:

```text
level
physical_duplicates
identity_conflict
duplicate_messages_threshold_reached
duplicate_groups_threshold_reached
max_copies_threshold_reached
```

The snapshot does not expose source counts or threshold values. It also excludes broker endpoints, stream/topic/partition/offset, UUIDs, payloads or digests, receipt identities, error classifications, credentials, timestamps, publisher identity, and raw client errors.

No serialization or persistence is added. A telemetry or health adapter must preserve this projection.

## Stable errors

```text
iggy.dlq_duplicate.alert_runtime_generation_overflow
iggy.dlq_duplicate.alert_runtime_publisher_closed
```

Generation overflow fails before replacing the current snapshot. Publisher closure is bounded and contains no runtime identifiers.

## Runtime boundaries

This reusable module does not:

- start a server worker;
- choose scan cadence or partitions;
- connect to Iggy;
- read poison receipts;
- register metrics or health checks;
- choose notification destinations;
- page, suppress, cool down, or deduplicate alerts;
- affect readiness;
- authorize Profiles presentation;
- acknowledge, delete, purge, replay, retry, or publish broker messages;
- claim or mark poison receipts;
- change broker configuration or profile state.

Observation, policy evaluation, latest-value publication, telemetry projection, notification delivery, and destructive workflows remain separate owner boundaries.

## Mode-aware server composition

The server now owns a separate integration:

```text
apps/server/src/services/event_dlq_duplicate_alert_observer.rs
```

It handles every active delivery profile explicitly:

```text
memory        -> not applicable, no Iggy access
outbox        -> not applicable, no Iggy access
outbox_iggy   -> bundled or external read-only Iggy observer
```

The observer is default-off. When active on `outbox_iggy`, it obtains bounded summaries from `IggyDlqDuplicateAlertObserver`, calls `publish` after successful scans, and calls `mark_unavailable` after connection failure, scan failure, or shutdown.

For bundled Iggy it connects to the already-running loopback broker. For external Iggy it uses the reviewed configured address list. It never creates another `IggyTransport` or starts another bundled process.

The server stores a read-only subscriber in `EventDlqDuplicateAlertObserverHandle`. Event delivery and module projection continue when observation is unavailable.

Complete mode, environment, privacy, and lifecycle rules are documented in:

```text
crates/rustok-iggy/docs/dlq-duplicate-alert-server-observer.md
```

## Source contracts

Reusable runtime:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-runtime-source.json
```

Server observer:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-server-observer-source.json
```

Static guards:

```bash
node scripts/verify/verify-iggy-dlq-duplicate-alert-runtime.mjs
node scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs
```

Focused runtime tests cover initial unavailability, successful publication, stale-evaluation clearing, independent subscribers, and bounded publisher closure. Server and observer tests cover all delivery profiles, both Iggy modes, partition bounds, and identifier-free stable errors.

No tests, Cargo commands, formatters, source verifiers, server observers, external-Iggy scans, telemetry registration, or alert dispatch were run while defining these source slices.

## Remaining work

1. project the identifier-free snapshot into reviewed telemetry and optional operational health;
2. retain runtime integration evidence for applicable Iggy modes;
3. define notification delivery, cooldown, and suppression outside this module;
4. define destructive reconciliation as a separately authorized workflow;
5. correlate receipt and duplicate aggregate health without exporting identifiers.
