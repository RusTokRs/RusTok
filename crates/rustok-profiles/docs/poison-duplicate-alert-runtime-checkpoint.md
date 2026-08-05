# Profiles checkpoint: DLQ duplicate alert runtime

Status: **identifier-free latest-value runtime and mode-aware server observer source-complete; runtime execution pending**.

## What changed

`rustok-iggy` owns a latest-value runtime composition for the count-only physical DLQ duplicate alert policy, and the server now owns a separate observer lifecycle that uses it only when the active event-delivery profile has an Iggy capability.

Reusable runtime source:

```text
crates/rustok-iggy/src/dlq_duplicate_alert_runtime.rs
```

Server observer sources:

```text
crates/rustok-iggy/src/dlq_duplicate_alert_observer.rs
apps/server/src/services/event_dlq_duplicate_alert_observer.rs
```

Machine contracts:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-runtime-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-server-observer-source.json
```

Verifiers:

```text
scripts/verify/verify-iggy-dlq-duplicate-alert-runtime.mjs
scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs
```

No Profiles API, storage, policy port, GraphQL field, storefront behavior, or authorization input changed.

## Delivery-profile boundary

The server observer handles all event-delivery profiles explicitly:

```text
outbox_local  -> NotApplicableOutboxLocal
outbox_iggy   -> IggyBundled or IggyExternal
```

`outbox_local` does not request an Iggy transport, does not open a broker connection, and does not require alert threshold configuration. Its not-applicable state is valid platform operation, not a Profiles degradation.

Only `outbox_iggy` resolves the shared `IggyTransport` already created by the event runtime. The observer never creates a second transport or bundled broker process.

## Composition boundary

The reusable publisher accepts only:

```text
already observed DlqDuplicateSummary
prevalidated DlqDuplicateAlertPolicy
```

The server's Iggy-specific source obtains the summary through bounded explicit-offset polling with `auto_commit=false`. It supports both the existing bundled loopback deployment and reviewed external addresses.

The single writer evaluates the summary and replaces one Tokio watch latest-value snapshot. Read-only subscribers may observe the current value or await a change.

## Initial and unavailable state

The initial snapshot is:

```text
generation = 0
available = false
evaluation = None
```

Every successful publication or unavailable transition increments generation through checked arithmetic.

Connection failure, scan failure, and shutdown call `mark_unavailable()`, which clears evaluation. This prevents a stale Warning or Critical result from appearing current while event delivery and module projection continue.

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

- event delivery profile;
- observer mode or applicability;
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

The runtime and server observer do not:

- change readiness or liveness;
- stop event delivery or module projection;
- emit notification delivery, paging, cooldown, suppression, or escalation timing;
- acknowledge, delete, purge, replay, retry, or publish broker messages;
- store consumer offsets;
- claim, release, or mark poison receipts;
- change broker configuration;
- change profile state.

Telemetry and optional operational health may consume the shared snapshot later, but they require separate contracts and must remain independent of readiness and Profiles authorization.

## Stable errors

```text
iggy.dlq_duplicate.alert_runtime_generation_overflow
iggy.dlq_duplicate.alert_runtime_publisher_closed
iggy.dlq_duplicate.alert_observer_configuration_invalid
iggy.dlq_duplicate.alert_observer_connection_unavailable
```

All are identifier-free and do not reveal broker or delivery facts.

## Remaining work

1. define reviewed telemetry and optional operational health projection;
2. retain server observer execution evidence for applicable Iggy modes;
3. keep notification delivery and suppression outside the runtime;
4. keep destructive reconciliation separately authorized;
5. preserve the rule that operational alert state never authorizes Profiles presentation.

Tests, Cargo commands, formatters, source verifiers, server observers, external-Iggy scans, telemetry registration, and alert dispatch were not run by the implementation agent.
