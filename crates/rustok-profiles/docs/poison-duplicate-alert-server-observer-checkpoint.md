# Profiles checkpoint: mode-aware physical DLQ duplicate alert observer

Status: **global and fair-window server composition source complete; runtime execution pending**.

## What changed

The host has one global event-delivery observer composition for the physical DLQ duplicate alert path:

```text
bounded physical DLQ scan
  -> DlqDuplicateSummary
  -> explicit DlqDuplicateAlertPolicy
  -> DlqDuplicateAlertRuntimePublisher
  -> identifier-free latest snapshot
```

The observer is intentionally not a Profiles service. Profiles remains a consumer of authoritative privacy and relationship owner ports only.

## Delivery and startup modes are explicit

```text
disabled      -> Disabled
startup issue -> Unavailable, no task or snapshot
memory        -> NotApplicableMemory
outbox_local  -> NotApplicableOutboxLocal
outbox_iggy   -> IggyBundled or IggyExternal
```

For `memory` and `outbox_local`:

- no `IggyTransport` is requested;
- no broker connection is opened;
- no alert thresholds are required;
- not-applicable state is not an error or degraded Profiles condition.

For `outbox_iggy`, the observer uses the exact shared transport configuration activated by the event runtime. It does not create another transport or broker. Missing active Iggy mode fails closed rather than being guessed.

Observer-specific startup errors are non-fatal. Invalid observer configuration or a missing observer dependency records `Unavailable`, logs only a stable code, and returns success to server bootstrap. Event delivery and module projection continue.

```text
iggy.dlq_duplicate.alert_server_observer_configuration_invalid
iggy.dlq_duplicate.alert_server_observer_runtime_unavailable
```

## Bundled and external Iggy

- bundled mode connects to the existing validated loopback broker and matching TCP port;
- external mode connects to reviewed configured addresses and credentials.

The observer never starts or owns the bundled process and never shuts down the shared event transport.

## Bounded scan semantics

The observer builds an allowlist containing every configured domain partition and uses explicit-offset, `auto_commit=false` polling.

Two scan modes are available:

```text
global_budget  -> one compatibility budget across the ordered allowlist
fair_window    -> one equal budget for every configured partition
```

`global_budget` remains the default, so existing deployments do not silently change semantics.

`fair_window` requires an explicit `RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_PER_PARTITION_MESSAGES`. The scanner checks the total budget against 10,000 messages and combines all partition observations before classification. A successful fair-window scan therefore attempts every configured partition under the same cap and preserves cross-partition duplicate or identity-conflict groups.

The fair mode is one fixed snapshot only. Every poll reuses the same configured start offset. It does not provide a moving cursor, stored offsets, cross-cycle duplicate accumulation, current-tail coverage, or complete-history proof.

No scan mode may become a Profiles input.

## Activation and policy

The observer is default-off:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_ENABLED=false
```

The scan mode is explicit:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=global_budget
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=fair_window
```

When active on `outbox_iggy`, all warning and critical thresholds must be supplied explicitly. Invalid values enter non-fatal `Unavailable` state; the library and server do not invent production tolerance defaults.

## Shared operational projection

`EventDlqDuplicateAlertObserverHandle` exposes only:

- the explicit observer mode;
- whether the task has finished;
- the latest optional `DlqDuplicateAlertRuntimeSnapshot`.

Startup-unavailable state has no task and no snapshot. After a task starts, connection failure, scan failure, or shutdown clears runtime availability and prior evaluation.

This state may later feed telemetry or an operational health endpoint, but it must not become bootstrap, readiness, or authorization input.

## Profiles authorization boundary

No profile visibility, ownership, follower access, relationship, block, mute, audience, storefront presentation, author card, or privacy-port result may depend on:

- event delivery profile;
- observer startup, applicability, availability, or generation;
- Iggy bundled/external mode;
- global or fair scan selection;
- partition ordering, fairness, budget exhaustion, or fixed-window coverage;
- duplicate alert level or threshold booleans;
- scanner connection state;
- retained observer evidence.

A missing or failed observer never changes Profiles data access. Event delivery and module projection remain active.

## Privacy boundary

The observer does not expose broker addresses/credentials, stream coordinates, UUIDs, payloads/digests, poison receipt identities, raw client errors, raw thresholds, or source counts.

Logs and shared state retain only stable codes, mode, availability, generation, alert level, and aggregate boolean reasons.

## Mutation boundary

The observer cannot publish/acknowledge messages, commit offsets, delete/purge/replay/retry DLQ entries, mutate poison receipts, start/stop bundled Iggy, change event delivery, dispatch notifications, or alter Profiles state.

## Source ownership

```text
crates/rustok-iggy/src/dlq_duplicate_alert_observer.rs
apps/server/src/services/event_dlq_duplicate_alert_observer.rs
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-server-observer-source.json
scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs
```

## Remaining work

1. execute and retain fair-window external-Iggy evidence;
2. design moving windows plus bounded cross-cycle duplicate state, or keep fixed windows;
3. add identifier-free telemetry projection;
4. add optional operational health without readiness coupling;
5. define notification routing, cooldown, and suppression separately;
6. retain execution evidence for applicable Iggy and unavailable startup modes;
7. keep destructive reconciliation separately authorized.

Tests, Cargo commands, formatters, verifiers, server startup, broker connections, alert delivery, and retained capture were not run by the implementation agent.
