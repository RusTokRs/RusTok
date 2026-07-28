# Mode-aware physical DLQ duplicate alert observer

Status: **server and Iggy source complete; runtime execution pending**.

## Purpose

The server composes the bounded physical DLQ scanner, explicit count-only alert policy, and latest-value runtime without assuming every event-delivery profile uses Iggy.

The observer is global event-delivery infrastructure. It is not owned by Social Graph, Profiles, or any one module because the physical `dlq` topic is transport-wide.

## Delivery-profile matrix

| Active profile | Observer mode | Iggy required |
|---|---|---|
| disabled | `Disabled` | no |
| startup/config unavailable | `Unavailable` | no task |
| `memory` | `NotApplicableMemory` | no |
| `outbox_local` | `NotApplicableOutboxLocal` | no |
| `outbox_iggy` + bundled | `IggyBundled` | yes |
| `outbox_iggy` + external | `IggyExternal` | yes |

`Memory` and `OutboxLocal` are not failures. The server records an intentional not-applicable mode and exits before asking for `Arc<IggyTransport>`.

Only `OutboxIggy` resolves the shared transport created by `build_event_runtime`. A missing active Iggy mode is internally inconsistent and fails closed rather than defaulting to external.

## Activation and startup isolation

The observer is default-off:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_ENABLED=false
```

It also requires a runtime profile that runs background workers.

When disabled, the server stores `Disabled` and starts no task or broker client. When enabled in a non-Iggy delivery profile, it stores the corresponding `NotApplicable` state and requires no thresholds.

Observer-specific startup failures do **not** fail application bootstrap. Invalid enable/configuration values, a missing active Iggy mode, or a missing shared observer dependency produce `Unavailable` with no task or snapshot and return normally to the bootstrap caller.

Stable startup codes:

```text
iggy.dlq_duplicate.alert_server_observer_configuration_invalid
iggy.dlq_duplicate.alert_server_observer_runtime_unavailable
```

Raw configuration values and raw dependency errors are not logged by this path. Event delivery and module projection continue.

## Iggy deployment modes

### Bundled

The observer does not start another broker process. It connects to the already-running loopback endpoint through the credentials paired with the bundled connector.

The configured address must be one loopback address using the configured bundled TCP port and plaintext TCP, matching the bundled connector contract.

### External

The observer tries the reviewed external addresses in configured order. Credentials and TLS options are used to construct SDK connection strings but are never retained in the runtime snapshot or logs.

## Scan modes

The scan mode is explicit:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=global_budget
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=fair_window
```

`global` and `fair` are accepted aliases. The default remains `global_budget`, preserving the previously published server behavior.

Common controls:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_POLL_MS       default 30000, max 300000
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_START_OFFSET  default 0
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_BATCH_SIZE    default 100
```

Both modes use:

```text
topic = dlq
standalone consumer
explicit start offset
configured one-based partition allowlist
auto_commit = false
```

### Compatibility global budget

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_MAX_MESSAGES  default 1000
```

One bounded budget is consumed across the ordered partition allowlist. A busy earlier partition may exhaust it before later partitions are polled.

### Fair snapshot window

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_PER_PARTITION_MESSAGES  required
```

Every configured partition receives the same message cap and is attempted on a successful scan. The scanner validates:

```text
partition_count * per_partition_messages <= 10000
batch_size <= per_partition_messages
partition_count <= 128
batch_size <= 1000
```

Observations from all partitions are combined before classification, so a repeated deterministic header UUID or conflicting payload group spanning partitions remains visible in the aggregate summary.

Fairness is limited to one fixed snapshot window. Every poll still reuses the same configured start offset. Neither mode adds:

- a moving cursor;
- persisted offsets;
- cross-cycle identity or digest accumulation;
- current-tail coverage;
- complete-history proof.

Moving a partition window without retaining bounded prior identity state could split copies across cycles and hide their relationship. That remains a separate reviewed design.

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

There are no production threshold defaults. Validation is delegated to `DlqDuplicateAlertPolicy` and fails closed into non-fatal `Unavailable` startup state.

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
- become a server-bootstrap, readiness, or liveness dependency;
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

1. execute fair-window scans against reviewed external Iggy partitions and retain evidence;
2. design moving per-partition windows with bounded cross-cycle duplicate state, or explicitly reject them;
3. project the shared snapshot into telemetry without exporting identifiers;
4. expose optional operational health without readiness coupling;
5. define alert routing, cooldown, and suppression separately;
6. retain server-observer execution evidence for each applicable mode and unavailable startup state;
7. keep destructive reconciliation in a separately authorized workflow.
