# Mode-aware physical DLQ duplicate alert observer

Status: **server and Iggy source complete; runtime execution pending**.

## Purpose

The server composes the bounded physical DLQ scanner, explicit count-only alert policy, and latest-value runtime without assuming every event-delivery profile uses Iggy.

The observer is global event-delivery infrastructure. It is not owned by Social Graph, Profiles, or any one module because the physical `dlq` topic is transport-wide.

## Delivery-profile matrix

| Active profile | Observer mode | Iggy required |
|---|---|---|
| `memory` | `NotApplicableMemory` | no |
| `outbox_local` | `NotApplicableOutboxLocal` | no |
| `outbox_iggy` + bundled | `IggyBundled` | yes |
| `outbox_iggy` + external | `IggyExternal` | yes |

`Memory` and `OutboxLocal` are not failures. The server records an intentional not-applicable mode and exits before asking for `Arc<IggyTransport>`.

Only `OutboxIggy` resolves the shared transport created by `build_event_runtime`. A missing active Iggy mode is an internally inconsistent runtime and fails closed rather than defaulting to external.

## Activation

The observer is default-off:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_ENABLED=false
```

It also requires a runtime profile that runs background workers.

When disabled, the server stores `Disabled` state and starts no task or broker client.

When enabled in a non-Iggy delivery profile, the server stores the corresponding `NotApplicable` state. Threshold variables are not required because no Iggy scan or policy evaluation can run.

## Iggy deployment modes

### Bundled

The observer does not start another broker process. It connects to the already-running loopback endpoint through the credentials paired with the bundled connector.

The configured address must be one loopback address using the configured bundled TCP port and plaintext TCP, matching the bundled connector contract.

### External

The observer tries the reviewed external addresses in configured order. Credentials and TLS options are used to construct SDK connection strings but are never retained in the runtime snapshot or logs.

## Scan configuration

Optional bounded controls:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_POLL_MS          default 30000, max 300000
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_START_OFFSET     default 0
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_MAX_MESSAGES     default 1000
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_BATCH_SIZE       default 100
```

The Iggy adapter builds a one-based allowlist containing every configured domain partition and passes it to the existing scanner:

```text
topic = dlq
standalone consumer
explicit start offset
configured partition allowlist
one bounded global message count
bounded batch size
auto_commit = false
```

The global message budget applies across the ordered partition allowlist. A busy earlier partition may exhaust the budget before later partitions are polled. This source slice therefore makes no partition-fairness or complete-history claim.

The scanner remains responsible for the hard limits: no more than 128 partition identifiers, 10,000 messages globally, or 1,000 messages per batch.

## Explicit alert thresholds

When the observer is active on `OutboxIggy`, all six threshold variables are required:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_WARNING_MESSAGES
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_CRITICAL_MESSAGES
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_WARNING_GROUPS
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_CRITICAL_GROUPS
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_WARNING_MAX_COPIES
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_CRITICAL_MAX_COPIES
```

There are no production threshold defaults. Validation is delegated to `DlqDuplicateAlertPolicy` and fails closed.

## Runtime lifecycle

The server creates one `DlqDuplicateAlertRuntimePublisher` and exposes its read-only subscriber through `EventDlqDuplicateAlertObserverHandle`.

A successful scan publishes an identifier-free available snapshot. Connection failure, scan failure, or shutdown advances generation, marks the snapshot unavailable, and clears the prior evaluation. Event delivery and module projection continue.

After connection or scan failure, the observer retries after the configured poll interval.

## Privacy boundary

The shared snapshot and logs contain only generation, availability, alert-level stable code, and aggregate boolean reasons.

They exclude broker addresses, stream/topic/partition/offset, UUIDs, payloads/digests, receipt identities, credentials, raw client errors, raw thresholds, and source counts.

## Lifecycle and mutation boundary

The observer does not:

- create or stop an `IggyTransport`;
- start or stop a bundled broker process;
- change readiness or liveness;
- stop event delivery or module projection;
- register notification routing, paging, cooldown, or suppression;
- publish, acknowledge, commit offsets, delete, purge, replay, or retry broker messages;
- claim or mark poison receipts;
- alter Profiles authorization.

## Source verification

```bash
node scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs
```

Tests, Cargo commands, formatters, source verifiers, server startup, Iggy connections, alert delivery, and retained capture were not run while authoring this slice.

## Remaining work

1. define partition fairness or an explicit per-partition budget policy;
2. project the shared snapshot into telemetry without exporting identifiers;
3. expose optional operational health without readiness coupling;
4. define alert routing, cooldown, and suppression separately;
5. retain server-observer execution evidence for each applicable mode;
6. keep destructive reconciliation in a separately authorized workflow.
