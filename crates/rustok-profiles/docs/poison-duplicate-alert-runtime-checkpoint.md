# Profiles checkpoint: DLQ duplicate alert runtime

Status: **identifier-free latest-value runtime source-complete; server integration pending**.

## What changed

`rustok-iggy` now owns an in-memory runtime composition for the count-only physical DLQ duplicate alert policy.

Source:

```text
crates/rustok-iggy/src/dlq_duplicate_alert_runtime.rs
```

Machine contract:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-runtime-source.json
```

Verifier:

```text
scripts/verify/verify-iggy-dlq-duplicate-alert-runtime.mjs
```

Public API:

```text
DlqDuplicateAlertRuntimePublisher
DlqDuplicateAlertRuntimeSubscriber
DlqDuplicateAlertRuntimeSnapshot
DlqDuplicateAlertRuntimeError
```

No Profiles API, storage, policy port, GraphQL field, storefront behavior, or authorization input changed.

## Composition boundary

The publisher accepts only:

```text
already observed DlqDuplicateSummary
prevalidated DlqDuplicateAlertPolicy
```

It does not scan Iggy, read PostgreSQL poison receipts, choose thresholds, or start a worker.

The single writer evaluates the summary and replaces one Tokio watch latest-value snapshot. Read-only subscribers may observe the current value or await a change.

## Initial and unavailable state

The initial snapshot is:

```text
generation = 0
available = false
evaluation = None
```

Every successful publication or unavailable transition increments generation through checked arithmetic.

`mark_unavailable()` always clears evaluation. This prevents a stale Warning or Critical result from appearing current after observer failure or shutdown.

## Identifier-free snapshot

The runtime snapshot exposes only:

```text
generation
available
evaluation
```

The evaluation exposes only:

```text
level
physical_duplicates
identity_conflict
duplicate_messages_threshold_reached
duplicate_groups_threshold_reached
max_copies_threshold_reached
```

It does not expose source counts, threshold values, broker endpoints, stream/topic/partition/offset, UUIDs, payloads or digests, receipt identities, error classifications, credentials, timestamps, or raw client errors.

The runtime adds no serialization or persistence.

## Profiles authorization remains unchanged

No profile visibility, owner access, relationship, follow, block, mute, friendship, audience, storefront, GraphQL, or presentation decision may depend on:

- runtime availability;
- runtime generation;
- alert level;
- threshold-reached booleans;
- identity-conflict state;
- publisher closure;
- future telemetry/health projection;
- retained runtime evidence.

Profiles continues to authorize through owner ports and its canonical privacy-before-presentation policy.

## Runtime and mutation boundary

The runtime does not:

- register readiness or health policy;
- emit metrics directly;
- choose notification delivery, paging, cooldown, suppression, or escalation timing;
- acknowledge, delete, purge, replay, retry, or publish broker messages;
- claim, release, or mark poison receipts;
- change broker configuration;
- change profile state.

A future server adapter may consume the snapshot only as operational observability. Notification and destructive workflows require separate owner contracts and authorization.

## Stable errors

```text
iggy.dlq_duplicate.alert_runtime_generation_overflow
iggy.dlq_duplicate.alert_runtime_publisher_closed
```

Both are identifier-free and do not reveal broker or delivery facts.

## Remaining work

1. integrate an explicit server-owned observer lifecycle;
2. define reviewed telemetry and health projection without source counts or identifiers;
3. retain runtime integration evidence;
4. keep notification delivery and suppression outside the runtime;
5. keep destructive reconciliation separately authorized;
6. preserve the rule that operational alert state never authorizes Profiles presentation.

Tests, Cargo commands, formatters, source verifiers, server observers, external-Iggy scans, telemetry registration, and alert dispatch were not run by the implementation agent.
