# Mode-aware physical DLQ duplicate alert observer

Status: **global, fixed fair-window, and moving-window server composition source complete; runtime execution pending**.

## Purpose

The server composes bounded physical DLQ scanning, count-only duplicate classification, explicit alert policy, and the latest-value runtime without assuming every event-delivery profile uses Iggy.

The observer is global event-delivery infrastructure. It is not owned by Social Graph, Profiles, or any one module because the physical `dlq` topic is transport-wide.

## Delivery-profile matrix

| Active profile | Observer mode | Iggy required |
|---|---|---|
| disabled | `Disabled` | no |
| startup/config unavailable | `Unavailable` | no task |
| `outbox` | `NotApplicableOutbox` | no |
| `outbox_iggy` + bundled | `IggyBundled` | yes |
| `outbox_iggy` + external | `IggyExternal` | yes |

`Outbox` is an intentional not-applicable mode. The server exits before asking for `Arc<IggyTransport>`. Only `OutboxIggy` resolves the shared transport created by the event runtime. A missing active Iggy mode fails closed rather than being guessed.

## Activation and startup isolation

The observer is default-off:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_ENABLED=false
```

It also requires a runtime profile that runs background workers. Observer-specific startup failures do not fail application bootstrap. Invalid enable/configuration values, a missing active Iggy mode, or a missing shared dependency produce `Unavailable`, start no task, and return normally.

Stable startup codes:

```text
iggy.dlq_duplicate.alert_server_observer_configuration_invalid
iggy.dlq_duplicate.alert_server_observer_runtime_unavailable
```

Raw configuration values and raw dependency errors are not logged. Event delivery and module projection continue.

## Iggy deployment modes

### Bundled

The observer connects to the already-running validated loopback broker. It does not start another broker process. The address must match the configured bundled TCP port and plaintext loopback contract.

### External

The observer tries reviewed external addresses in configured order. Credentials and TLS options are used only to construct SDK connection strings and are never retained in snapshots or logs.

## Scan-mode selection

The scan mode is explicit:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=global_budget
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=fair_window
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=moving_window
```

`global`, `fair`, and `moving` are accepted aliases. `global_budget remains the default`, so existing deployments do not silently change semantics.

Common lifecycle control:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_POLL_MS  default 30000, max 300000
```

All modes use the configured one-based domain partition allowlist, the physical `dlq` topic, a standalone consumer, explicit-offset polling, and `auto_commit=false`. No mode stores a broker consumer offset.

### Compatibility global budget

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_START_OFFSET  default 0
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_MAX_MESSAGES default 1000
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_BATCH_SIZE   default 100
```

One bounded budget is consumed across the ordered partition allowlist. A busy earlier partition may exhaust it before later partitions are polled. Every poll reuses the configured start offset and retains no cross-cycle state.

### Fixed fair window

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_START_OFFSET          default 0
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_PER_PARTITION_MESSAGES required
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_BATCH_SIZE             default min(100, per-partition cap)
```

Every configured partition receives the same message cap and is attempted during a successful scan. The checked total cannot exceed 10,000 messages. Observations are combined before one count-only classification, but every poll still reuses the configured start offset and retains no cross-cycle state.

### Moving window

Moving mode is an explicit opt-in with reviewed fail-closed configuration. It has no production defaults:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_START_OFFSET                         required
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_PER_PARTITION_MESSAGES               required
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_BATCH_SIZE                            required
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_ROLLING_MAX_CYCLES                    required
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_ROLLING_MAX_OBSERVATIONS_PER_CYCLE   required
```

Startup validation constructs `IggyDlqDuplicateAlertMovingWindowConfig` before the task starts. Validation requires:

```text
1 <= partition_count <= 128
partition_count * per_partition_messages <= 10000
batch_size <= min(1000, per_partition_messages)
rolling_max_cycles <= 128
rolling_max_cycles * rolling_max_observations_per_cycle <= 10000
rolling_max_observations_per_cycle >= complete fair-cycle budget
```

Every selected partition owns one private process-local next offset. One observer poll collects every partition into a temporary candidate. Only after the complete cycle is valid and accepted by `DlqDuplicateRollingWindow` are all private cursors and rolling state updated together.

A failed moving cycle marks the public alert runtime unavailable but retains the connected observer's process-local cursors and rolling history for the next attempt. Fixed modes remain reconnectable snapshots and rebuild after scan failure.

## Restart and reconnect boundary

Moving progress is deliberately not persisted. A newly connected observer, a process restart, or a replacement after connection failure starts from the reviewed initial offset with empty rolling history. This may reread an earlier bounded region.

The current source therefore does not claim:

- restart-safe progress;
- current-tail coverage;
- complete broker history;
- production retention sufficiency;
- exactly-once delivery.

A persistent cursor owner remains separate work only if an operator requirement justifies ownership, fencing, migration, and recovery semantics. Deployment review must explicitly choose the initial offset and acceptable reset frequency.

## Alert projection

All six warning and critical threshold variables remain required whenever the observer is active:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_WARNING_MESSAGES
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_CRITICAL_MESSAGES
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_WARNING_GROUPS
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_CRITICAL_GROUPS
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_WARNING_MAX_COPIES
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_CRITICAL_MAX_COPIES
```

Moving snapshots are reduced to the existing `DlqDuplicateSummary` before policy evaluation. The shared latest-value runtime API is unchanged.

A successful scan publishes an identifier-free available evaluation. Connection failure, scan failure, or shutdown advances runtime generation, marks the snapshot unavailable, and clears prior evaluation. Event delivery and module projection continue.

## Privacy boundary

The shared snapshot and logs contain only availability generation, alert-level stable code, aggregate reason booleans, and one boolean stating whether process-local moving state was preserved after a failed cycle.

They exclude broker addresses, stream/topic/partition/offset, private cursor values, UUIDs, payloads/digests, retained observations, receipt identities, credentials, raw client errors, raw thresholds, and source counts. The moving configuration's custom debug projection also excludes its initial offset.

## Lifecycle and mutation boundary

The observer does not:

- create or stop an `IggyTransport`;
- start or stop a bundled broker process;
- become a bootstrap, readiness, liveness, projection, or Profiles authorization dependency;
- register notification routing, paging, cooldown, or suppression;
- publish, acknowledge, commit offsets, delete, purge, replay, or retry broker messages;
- claim or mark poison receipts.

## Source verification

```bash
cargo test -p rustok-iggy dlq_duplicate_alert_observer --features iggy -- --nocapture
cargo test -p rustok-server event_dlq_duplicate_alert_observer -- --nocapture
node scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs
node scripts/verify/verify-iggy-dlq-duplicate-moving-window-scan.mjs
```

Tests, Cargo commands, formatters, source verifiers, server startup, Iggy connections, alert delivery, and retained capture were not run while authoring this source slice.

## Remaining work

1. execute fixed fair-window evidence against reviewed external Iggy;
2. retain a real duplicate split across advancing moving-window cycles;
3. review initial offset and acceptable reset frequency per deployment;
4. add a persistent cursor owner only if restart continuity is required;
5. project identifier-free telemetry and optional operational health without readiness coupling;
6. define notification routing, cooldown, and suppression separately;
7. retain server-observer execution evidence for applicable and unavailable modes;
8. keep destructive reconciliation in a separately authorized workflow.
