# Profiles checkpoint: mode-aware physical DLQ duplicate alert observer

Status: **server composition source complete; runtime execution pending**.

## What changed

The host now has one global event-delivery observer composition for the physical DLQ duplicate alert path.

It combines:

```text
bounded physical DLQ scan
  -> DlqDuplicateSummary
  -> explicit DlqDuplicateAlertPolicy
  -> DlqDuplicateAlertRuntimePublisher
  -> identifier-free latest snapshot
```

The observer is intentionally not a Profiles service. Profiles remains a consumer of authoritative privacy and relationship owner ports only.

## Delivery modes are explicit

The composition does not assume that every deployment uses Iggy:

```text
memory        -> NotApplicableMemory
outbox_local  -> NotApplicableOutboxLocal
outbox_iggy   -> IggyBundled or IggyExternal
```

For `memory` and `outbox_local`:

- no `IggyTransport` is requested;
- no broker connection is opened;
- no alert thresholds are required;
- not-applicable state is not an error or degraded Profiles condition.

For `outbox_iggy`, the observer uses the exact shared transport configuration already activated by the event runtime. It does not create another transport or broker.

## Bundled and external Iggy

Both Iggy deployment modes are handled explicitly:

- bundled mode connects to the existing validated loopback broker and matching TCP port;
- external mode connects to reviewed configured addresses and credentials.

The observer never starts or owns the bundled process and never shuts down the shared event transport.

## Activation and policy

The observer is default-off:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_ENABLED=false
```

When active on `outbox_iggy`, all warning and critical thresholds must be supplied explicitly. The library and server do not invent production tolerance defaults.

## Shared operational projection

`EventDlqDuplicateAlertObserverHandle` exposes only:

- the explicit observer mode;
- whether the task has finished;
- the latest optional `DlqDuplicateAlertRuntimeSnapshot`.

The snapshot contains generation, availability, and the identifier-free alert evaluation. On connection failure, scan failure, or shutdown, availability becomes false and the prior evaluation is cleared.

This state may later feed telemetry or an operational health endpoint, but it must not become readiness or authorization input.

## Profiles authorization boundary

No profile visibility, ownership, follower access, relationship, block, mute, audience, storefront presentation, author card, or privacy-port result may depend on:

- event delivery profile;
- observer applicability or availability;
- Iggy bundled/external mode;
- duplicate alert level;
- threshold booleans;
- scanner connection state;
- alert generation;
- retained observer evidence.

A missing or failed Iggy observer never changes Profiles data access. Event delivery and module projection remain active.

## Privacy boundary

The observer does not expose:

- broker addresses or credentials;
- stream/topic/partition/offset;
- deterministic message UUIDs;
- payloads or payload digests;
- poison receipt identities;
- raw client errors;
- raw threshold values;
- source counts.

Logs and shared state retain only stable codes, availability, generation, alert level, and aggregate boolean reasons.

## Mutation boundary

The observer cannot:

- publish or acknowledge messages;
- commit consumer offsets;
- delete, purge, replay, or retry DLQ entries;
- claim or mark poison receipts;
- start or stop the bundled Iggy process;
- change event delivery configuration;
- dispatch notifications;
- alter Profiles state.

## Source ownership

```text
crates/rustok-iggy/src/dlq_duplicate_alert_observer.rs
apps/server/src/services/event_dlq_duplicate_alert_observer.rs
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-server-observer-source.json
scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs
```

## Remaining work

1. add identifier-free telemetry projection;
2. add optional operational health without readiness coupling;
3. define notification routing, cooldown, and suppression separately;
4. retain execution evidence for applicable Iggy modes;
5. keep any destructive reconciliation separately authorized.

Tests, Cargo commands, formatters, verifiers, server startup, broker connections, alert delivery, and retained capture were not run by the implementation agent.
