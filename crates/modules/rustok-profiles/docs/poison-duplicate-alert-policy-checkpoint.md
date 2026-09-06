# Profiles checkpoint: physical DLQ duplicate alert policy

Status: **count-only policy, latest-value runtime, and mode-aware server observer source-complete; runtime execution pending**.

## What changed

`rustok-iggy` owns a transport-neutral policy that evaluates the count-only physical DLQ duplicate summary and an in-memory latest-value runtime for the resulting identifier-free evaluation. The host owns a separate mode-aware observer lifecycle.

Sources:

```text
policy:          crates/rustok-iggy/src/dlq_duplicate_alert_policy.rs
runtime:         crates/rustok-iggy/src/dlq_duplicate_alert_runtime.rs
Iggy observer:   crates/rustok-iggy/src/dlq_duplicate_alert_observer.rs
server observer: apps/server/src/services/event_dlq_duplicate_alert_observer.rs
```

Machine contracts:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-policy-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-runtime-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-server-observer-source.json
```

No Profiles API, database table, GraphQL field, storefront behavior, privacy port, or authorization input changed.

## Explicit operator thresholds

The policy requires explicit warning and critical thresholds for:

```text
duplicate_messages
duplicate_groups
max_copies_per_message_id
```

There is no library-owned production default. Invalid zero, inverted, or impossible max-copies thresholds fail closed.

Profiles does not select operational duplicate tolerance, and `rustok-iggy` does not invent a tenant or product policy.

## Level semantics

```text
Clear     no physical duplicate
Notice    duplicates exist below warning thresholds
Warning   one or more warning thresholds reached
Critical  one or more critical thresholds reached
          OR any identity conflict exists
```

Identity conflict is always `Critical` and requires manual review. A numeric `Critical` result without an identity conflict does not authorize payload inspection or destructive reconciliation.

## Latest-value runtime

```text
already observed DlqDuplicateSummary
  -> prevalidated DlqDuplicateAlertPolicy
  -> single-writer runtime publisher
  -> identifier-free latest snapshot
  -> read-only subscribers
```

Initial state is unavailable with generation `0` and no evaluation. Observation failure or shutdown publishes unavailable and clears the old evaluation so stale severity does not remain current.

The channel retains only the latest state. It is not an audit log.

## Delivery-profile-aware server composition

The observer handles all active event delivery profiles explicitly:

```text
outbox        -> NotApplicableOutbox
outbox_iggy   -> IggyBundled or IggyExternal
```

For `outbox`:

- no Iggy transport is requested;
- no broker client is opened;
- no alert thresholds are required;
- not-applicable state is not a Profiles degradation.

For `outbox_iggy`, the observer reuses the exact active Iggy configuration. Bundled mode connects to the existing loopback broker; external mode uses reviewed configured addresses. It never creates another transport or broker process. Missing active Iggy mode fails closed.

Connection and scan failures clear runtime availability but do not stop event delivery or module projection.

## Count-only runtime projection

The runtime snapshot exposes only:

```text
generation
available
evaluation
```

The optional evaluation exposes only level and aggregate boolean reasons. It excludes source counts, raw thresholds, broker coordinates, UUIDs, payloads/digests, receipt identities, credentials, timestamps, and raw Iggy errors.

No serialization or persistence is added.

## Profiles authorization boundary

No profile visibility, ownership, follower access, block, mute, relationship, audience, storefront presentation, or author-card decision may depend on:

- event delivery profile or Iggy deployment mode;
- observer applicability or availability;
- `DlqDuplicateAlertLevel`;
- runtime generation;
- threshold-reached booleans;
- physical duplicate or identity-conflict presence;
- scanner connection state;
- alert delivery state;
- retained evidence metadata.

Profiles continues to resolve privacy through authoritative owner ports and presents only approved owner results.

## Operational separation

The policy/runtime/observer path cannot:

- change readiness or liveness;
- stop event delivery or projection;
- send or page;
- choose a destination, cooldown, or suppression;
- persist thresholds or snapshots;
- acknowledge, delete, purge, replay, retry, or publish;
- store consumer offsets;
- mutate poison receipts;
- alter broker configuration or profile state.

Telemetry projection, optional health, notification delivery, cooldown/suppression, and destructive reconciliation remain separate owner boundaries.

## Remaining work

1. define reviewed telemetry and optional health without readiness coupling;
2. define alert routing, cooldown, and suppression outside Profiles and the policy/runtime;
3. retain identifier-free runtime/server execution evidence;
4. keep destructive reconciliation in a separate authorized workflow;
5. compare receipt and duplicate health only as aggregate operational trends.

No tests, Cargo commands, formatters, verifiers, server observers, broker connections, alert delivery, or retained capture were run by the implementation agent.
