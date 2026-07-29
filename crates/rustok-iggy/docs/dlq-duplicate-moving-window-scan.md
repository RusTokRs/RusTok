# Moving physical DLQ duplicate window scan

Status: **scanner, independent in-memory partition cursors, complete-cycle rolling integration, and explicit restart reset source-complete; server composition and runtime evidence pending**.

## Purpose

The fixed global and fair scanners classify one explicit-offset snapshot. The bounded rolling state retains opaque observations across complete cycles, but it does not collect those cycles or advance broker offsets.

`IggyDlqDuplicateMovingWindowScanner` and `IggyDlqDuplicateMovingWindowState` compose those boundaries without exporting identifiers:

```text
independent private partition cursors
  -> complete equal-budget read-only Iggy cycle
  -> one atomic rolling-window push
  -> identifier-free moving-window snapshot
```

The adapter is opt-in and does not replace the compatibility fixed-window scanner or the current server observer.

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
- the rolling policy's per-cycle capacity must cover the full fair-cycle budget;
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

The scanner validates returned partition, count, strictly increasing offsets, header UUID, and checked offset advancement.

Nothing is committed while polling. Only after every partition succeeds does the state:

1. validate the complete partition set and each expected start offset;
2. combine opaque observations without exposing them;
3. push one complete cycle into `DlqDuplicateRollingWindow`;
4. replace every private cursor together.

A polling, response, offset, classification, or rolling-window error leaves all cursors and rolling state unchanged. Empty partitions are successful and keep their cursor unchanged.

## Count-only result

`IggyDlqDuplicateMovingWindowSnapshot` exposes only:

```text
rolling
partition_count
advanced_partitions
reset_generation
```

`rolling` is the existing identifier-free `DlqDuplicateRollingWindowSnapshot`. The moving snapshot does not expose partition IDs, cursor values, offsets, UUIDs, payloads or digests, endpoints, credentials, receipt identities, timestamps, or raw Iggy errors.

## Restart and reset semantics

Progress persistence is deliberately **not** part of this source slice.

A newly constructed process-local state starts every partition at the reviewed `initial_offset` and starts with an empty rolling history. `reset_to_initial_offset()` performs the same transition explicitly and increments only a count-only reset generation.

Therefore:

- process restart may reread an earlier bounded region;
- no restart-continuity or current-tail claim is made;
- no cursor or rolling observation is serialized;
- a deployment that requires restart continuity must add a separately reviewed persistent cursor owner;
- `history_truncated` still means only that rolling cycles were evicted, not that broker history is complete.

## Mutation and authorization boundary

The moving scanner never stores consumer offsets, acknowledges, publishes, deletes, purges, retries, replays, mutates poison receipts, changes topology, or shuts down the caller's client.

Its state never authorizes profile visibility, relationships, blocks, follows, storefront rendering, GraphQL presentation, or any Social Graph behavior.

## Source verification

```bash
cargo test -p rustok-iggy dlq_duplicate_moving_window_scan --features iggy -- --nocapture
node scripts/verify/verify-iggy-dlq-duplicate-moving-window-scan.mjs
node scripts/verify/verify-iggy-dlq-duplicate-rolling-window.mjs
```

These commands were not run by the implementation agent.

## Remaining work

1. add an explicit opt-in mode to the mode-aware server observer;
2. define reviewed environment/configuration fields for initial offset, fair budget, batch size, and rolling bounds;
3. retain a real external-Iggy scenario with one duplicate split across advancing cycles;
4. add a persistent cursor owner only if restart continuity is required;
5. project identifier-free telemetry and optional operational health without readiness coupling.
