# Count-only DLQ duplicate alert runtime

Status: **latest-value runtime composition source-complete; server observer integration pending**.

## Purpose

`DlqDuplicateAlertRuntimePublisher` composes two already separated owner inputs:

```text
DlqDuplicateSummary
DlqDuplicateAlertPolicy
```

It evaluates an already-observed count-only summary and publishes one identifier-free latest-value snapshot for telemetry or health consumers.

The runtime does not scan Iggy, read poison receipts, choose thresholds, dispatch notifications, or perform reconciliation.

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

No serialization or persistence is added. A future telemetry or health adapter must preserve this projection.

## Stable errors

```text
iggy.dlq_duplicate.alert_runtime_generation_overflow
iggy.dlq_duplicate.alert_runtime_publisher_closed
```

Generation overflow fails before replacing the current snapshot. Publisher closure is bounded and contains no runtime identifiers.

## Runtime boundaries

This module does not:

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

## Suggested server composition

A future server-owned observer should:

```text
1. obtain a bounded DlqDuplicateSummary from an approved observer;
2. call publisher.publish(summary) after successful observation;
3. call publisher.mark_unavailable() after observation failure or shutdown;
4. expose a subscriber only to count-free telemetry/health adapters;
5. keep projection/readiness and Profiles authorization independent;
6. leave paging, cooldown, suppression, and destructive actions outside this runtime.
```

The server integration must decide configuration ownership and lifecycle separately. This source slice intentionally provides no production defaults or environment variables.

## Source contract

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-runtime-source.json
```

Static guard:

```bash
node scripts/verify/verify-iggy-dlq-duplicate-alert-runtime.mjs
```

Focused source tests cover initial unavailability, successful publication, stale-evaluation clearing, independent subscribers, and bounded publisher closure.

No tests, Cargo commands, formatters, source verifiers, server observers, external-Iggy scans, telemetry registration, or alert dispatch were run while defining this source slice.

## Remaining work

1. integrate one explicit server-owned observer lifecycle;
2. project the identifier-free snapshot into reviewed telemetry and health contracts;
3. retain runtime integration evidence;
4. define notification delivery, cooldown, and suppression outside this module;
5. define destructive reconciliation as a separately authorized workflow;
6. correlate receipt and duplicate aggregate health without exporting identifiers.
