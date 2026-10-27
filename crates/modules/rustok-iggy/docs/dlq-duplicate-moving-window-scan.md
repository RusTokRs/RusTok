# Moving physical DLQ duplicate window scan

Status: **scanner, independent process-local partition cursors, complete-cycle rolling integration, explicit restart reset, and server composition source-complete; external runtime evidence pending**.

## Purpose

The fixed global and fair scanners classify one explicit-offset snapshot. `IggyDlqDuplicateMovingWindowScanner` and `IggyDlqDuplicateMovingWindowState` retain opaque relationships across advancing cycles without exporting identifiers:

```text
independent private partition cursors
  -> complete equal-budget read-only Iggy cycle
  -> one atomic rolling-window push
  -> identifier-free moving-window snapshot
  -> count-only server alert input
```

The mode is opt-in. `global_budget` remains the server default.

## Explicit policy

`IggyDlqDuplicateMovingWindowPolicy` requires:

```text
partitions
initial_offset
per_partition_messages
batch_size
DlqDuplicateRollingWindowPolicy
```

Rules:

- 1 to 128 unique positive partitions;
- positive equal per-partition message budget;
- batch size no greater than 1,000 or the per-partition budget;
- checked total `partition_count * per_partition_messages <= 10000`;
- rolling per-cycle capacity covers the full fair-cycle budget;
- no production defaults.

Every partition begins at the same reviewed `initial_offset`, then advances independently in process memory.

## Complete-cycle atomicity

One scan cycle polls every configured partition with:

```text
topic = dlq
standalone consumer
explicit partition
PollingStrategy::offset(private_next_offset)
auto_commit = false
```

Nothing is committed while polling. Only after every partition succeeds does the state:

1. validate the complete partition set and each expected start offset;
2. combine opaque observations without exporting them;
3. push one complete cycle into `DlqDuplicateRollingWindow`;
4. replace every private cursor together.

A polling, response, offset, classification, or rolling-window error leaves all private cursor and rolling state unchanged. Empty partitions are successful and keep their cursor unchanged.

## Count-only result

`IggyDlqDuplicateMovingWindowSnapshot` exposes only:

```text
rolling
partition_count
advanced_partitions
reset_generation
```

`rolling` is the existing identifier-free `DlqDuplicateRollingWindowSnapshot`. Partition IDs, cursor values, offsets, UUIDs, payloads/digests, endpoints, credentials, receipt identities, timestamps, and raw Iggy errors are excluded.

## Restart and reset semantics

Progress persistence is deliberately **not** part of this boundary.

A newly constructed process-local state starts every partition at the reviewed initial offset and starts with empty rolling history. `reset_to_initial_offset()` performs the same transition explicitly and increments only a count-only reset generation.

Therefore:

- process or replacement connection may reread an earlier bounded region;
- no restart-continuity or current-tail claim is made;
- no cursor or rolling observation is serialized;
- a persistent cursor owner is separate work only if restart continuity is required;
- `history_truncated` reports rolling-cycle eviction, not complete broker history.

## Server composition

The server composition is source-complete through:

```text
IggyDlqDuplicateAlertMovingWindowConfig
IggyDlqDuplicateAlertObserver::connect_moving_window
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=moving_window
```

`moving` is an accepted alias. Server startup requires explicit reviewed values for initial offset, per-partition budget, batch size, rolling maximum cycles, and rolling maximum observations per cycle. Invalid or incomplete values fail closed into the observer's non-fatal `Unavailable` startup mode.

The Iggy observer retains moving state inside its scan enum. `summarize(&mut self)` executes one complete moving cycle and reduces the result to the existing count-only `DlqDuplicateSummary` before alert evaluation.

A failed moving cycle keeps the connected observer and retries the same process-local cursors and rolling history. Fixed global/fair failures continue to rebuild reconnectable snapshots. A replacement connection still starts at the reviewed initial offset because durable progress is not claimed.

## Mutation and authorization boundary

The moving scanner and observer never store consumer offsets, acknowledge, publish, delete, purge, retry, replay, mutate poison receipts, change topology, or shut down the caller's client or shared transport.

Their state never authorizes profile visibility, relationships, blocks, follows, storefront rendering, GraphQL presentation, or Social Graph behavior.

## Source verification

```bash
cargo test -p rustok-iggy dlq_duplicate_moving_window_scan --features iggy -- --nocapture
cargo test -p rustok-iggy dlq_duplicate_alert_observer --features iggy -- --nocapture
cargo test -p rustok-server event_dlq_duplicate_alert_observer -- --nocapture
node scripts/verify/verify-iggy-dlq-duplicate-moving-window-scan.mjs
node scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs
```

These commands were not run by the implementation agent.

## Remaining work

1. retain a real external-Iggy scenario with one duplicate split across advancing cycles;
2. review initial offset and acceptable reset frequency per deployment;
3. add a persistent cursor owner only if restart continuity is required;
4. retain server observer execution evidence;
5. project identifier-free telemetry and optional operational health without readiness coupling.
