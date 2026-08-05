# Profiles checkpoint: mode-aware physical DLQ duplicate alert observer

Status: **global, fixed fair-window, and moving-window server composition source complete; runtime execution pending**.

## What changed

The host now has one explicit event-delivery observer composition for all three physical DLQ duplicate scan modes:

```text
global_budget | fair_window | moving_window
  -> DlqDuplicateSummary
  -> explicit DlqDuplicateAlertPolicy
  -> DlqDuplicateAlertRuntimePublisher
  -> identifier-free latest snapshot
```

The observer is not a Profiles service. Profiles continues to consume authoritative privacy, Media, and relationship owner ports only.

## Delivery and startup modes

```text
disabled      -> Disabled
startup issue -> Unavailable, no task or snapshot
outbox_local  -> NotApplicableOutboxLocal
outbox_iggy   -> IggyBundled or IggyExternal
```

For `outbox_local`, no shared Iggy transport is requested, no broker connection is opened, and no alert thresholds are required. For `outbox_iggy`, the observer reuses the exact active transport configuration. Missing active mode or invalid observer configuration fails closed into non-fatal `Unavailable` state.

Event delivery and module projection remain active.

## Scan modes

`global_budget` remains the compatibility default. `fair_window` and `moving_window` are explicit opt-ins.

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=global_budget
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=fair_window
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=moving_window
```

All modes poll the physical `dlq` topic through a standalone consumer, explicit offsets, and `auto_commit=false`. No broker-stored consumer offset is created.

### Global and fixed fair modes

Global mode shares one bounded budget across the ordered allowlist. Fixed fair mode gives every configured partition one equal cap. Both reuse one configured start offset on every poll and retain no cross-cycle identity state.

### Moving mode

Moving mode composes the source-complete moving scanner and rolling state. It requires reviewed fail-closed configuration for:

```text
initial offset
per-partition message cap
batch size
rolling maximum cycles
rolling maximum observations per cycle
```

There are no production defaults for these moving controls. Validation requires the full checked fair-cycle budget to fit one rolling cycle.

Every partition owns one private cursor in process memory. A complete cycle is collected before mutation. All private cursors and rolling history update only after every partition succeeds and the combined rolling cycle is accepted.

A failed cycle marks the alert runtime unavailable but preserves the connected moving observer's private cursor and rolling state for the next attempt. The public alert projection still receives only the count-only `DlqDuplicateSummary`.

## Restart choice

The server composition keeps the explicit reset choice from the moving scanner:

- cursors and rolling observations are process-local;
- a new connection or process starts from the reviewed initial offset;
- replacement after connection failure starts with empty rolling history;
- rereading an earlier bounded region is allowed;
- no restart-safe progress, current-tail, complete-history, or exactly-once claim is made.

A persistent cursor store remains a separate owner only if an operator requirement justifies its fencing and recovery semantics. Deployment review must choose the initial offset and acceptable reset frequency.

## Shared operational projection

`EventDlqDuplicateAlertObserverHandle` still exposes only:

- explicit observer applicability/deployment mode;
- whether its task has finished;
- the latest optional identifier-free `DlqDuplicateAlertRuntimeSnapshot`.

The runtime snapshot contains availability, generation, alert level, and aggregate boolean reasons. Moving-window partition IDs, private cursor values, offsets, UUIDs, payloads/digests, rolling observations, credentials, raw errors, raw thresholds, and source counts are excluded.

## Profiles authorization boundary

No profile visibility, ownership, follower access, relationship, block, mute, audience, storefront presentation, author card, privacy-port result, ranking, or mutation may depend on:

- observer startup, availability, generation, or scan mode;
- Iggy bundled/external deployment mode;
- fixed or moving cursor behavior;
- private cursor advancement or reset;
- rolling retention or `history_truncated`;
- duplicate counts, identity-conflict flags, or alert levels;
- retained execution evidence.

A missing or failed observer never changes Profiles data access. Operational state remains operational only.

## Mutation boundary

The observer cannot publish or acknowledge messages, store broker offsets, delete/purge/replay/retry DLQ entries, mutate poison receipts, start/stop bundled Iggy, change event delivery, dispatch notifications, or alter Profiles state.

## Source ownership

```text
crates/rustok-iggy/src/dlq_duplicate_alert_observer.rs
crates/rustok-iggy/src/dlq_duplicate_moving_window_scan.rs
apps/server/src/services/event_dlq_duplicate_alert_observer.rs
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-server-observer-source.json
scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs
```

## Remaining work

1. execute and retain fixed fair-window external-Iggy evidence;
2. retain real moving-window external-Iggy cross-cycle evidence;
3. review initial offset and reset frequency per deployment;
4. add persistent cursor ownership only if restart continuity is required;
5. add identifier-free telemetry and optional health without readiness coupling;
6. define notification routing and suppression separately;
7. retain execution evidence for applicable and unavailable modes;
8. keep destructive reconciliation separately authorized.

Tests, Cargo commands, formatters, verifiers, server startup, broker connections, alert delivery, and retained capture were not run by the implementation agent.
