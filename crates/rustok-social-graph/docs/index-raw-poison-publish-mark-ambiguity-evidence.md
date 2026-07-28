# Social Graph raw-poison publish/mark ambiguity evidence

Status: **source complete; external PostgreSQL/Iggy execution pending**.

## Boundary under test

The Social Graph Index worker cannot atomically commit an Iggy DLQ publish and a PostgreSQL poison-receipt transition. Its approved order is:

```text
reserve_and_claim
IggyTransport::move_to_dlq
mark_published
acknowledge source offset
best-effort mark_acknowledged
```

A process may stop after Iggy accepted the deterministic DLQ message but before PostgreSQL reached `published`. PostgreSQL then still reports `publishing`, the source offset is uncommitted, and a later publisher may reclaim the receipt after the durable lease expires.

This harness isolates that ambiguity window. It does not describe it as a PostgreSQL/Iggy transaction.

## Source harness

Test target:

```text
crates/rustok-social-graph/tests/index_raw_poison_publish_mark_ambiguity.rs
```

Feature:

```text
index-consumer
```

Each case creates:

- one unique PostgreSQL schema;
- one connection per SeaORM pool;
- one unique external-Iggy stream;
- one `domain` partition and one `dlq` partition;
- one production `SocialGraphIndexConsumer` at a time;
- one read-only Iggy SDK observer that calls only `get_topic` and reads partition `messages_count`.

The connector fixture only injects malformed source bytes. All source receive/redelivery, deterministic DLQ publication, receipt transitions, and source acknowledgement use public production APIs.

## Shared PostgreSQL input

```text
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_DATABASE_URL=postgresql://...
```

There is no `DATABASE_URL` fallback. The test creates and drops only its own unique schema.

## Dedup-enabled case

```text
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_DEDUP_ENABLED_ADDRESS=host:port
```

Exact case:

```text
dedup_enabled_closes_publish_mark_ambiguity_without_physical_duplicate
```

The external broker must exhibit enabled message-ID deduplication for the bounded scenario. The physical DLQ counts must be:

```text
0 -> 1 -> 1
```

The first publisher:

1. receives malformed bytes without acknowledging the source;
2. claims a one-second PostgreSQL lease;
3. publishes through `IggyTransport::move_to_dlq`;
4. leaves the receipt in `publishing`;
5. shuts down before `mark_published` and source acknowledgement.

After a bounded 1.5-second wait, the recovery publisher receives the same source offset, deterministic delivery UUID, and exact raw bytes. It reclaims the expired receipt and retries the same deterministic broker message ID. The broker retains one physical DLQ message.

The recovery publisher then persists `published`, acknowledges the source, records `acknowledged`, and receives the next source offset.

## Dedup-disabled case

```text
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_DEDUP_DISABLED_ADDRESS=host:port
```

Exact case:

```text
dedup_disabled_exposes_publish_mark_ambiguity_as_physical_duplicate
```

The physical DLQ counts must be:

```text
0 -> 1 -> 2
```

The PostgreSQL and source-cursor sequence is identical to the enabled case. The same deterministic message ID is retried after lease recovery, but a broker without deduplication accepts a second physical DLQ message.

This is an intentional negative proof: durable receipt fencing prevents two live publishers from owning the lease simultaneously, but it cannot erase the cross-system ambiguity after the broker has succeeded and PostgreSQL has not yet recorded `published`.

## Credentials

Optional shared credentials must be supplied as a pair:

```text
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_IGGY_USERNAME=...
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_IGGY_PASSWORD=...
```

The two mode addresses must be distinct when both are present. Addresses must be bounded `host:port` values without schemes, embedded credentials, query strings, or fragments.

TLS, failover, and bundled-Iggy behavior are outside this source slice.

## Suggested maintainer execution

Run the source verifier first:

```bash
node scripts/verify/verify-social-graph-index-raw-poison-publish-mark-ambiguity.mjs
```

Then run both external cases serially:

```bash
cargo test -p rustok-social-graph --features index-consumer \
  --test index_raw_poison_publish_mark_ambiguity \
  -- --nocapture --test-threads=1
```

The harness is opt-in. Missing PostgreSQL or scenario-specific Iggy inputs produce an explicit skip message; a future retained runner must reject those skips.

## What the source proves when executed successfully

- a live second publisher observes `Busy` before the first lease expires;
- natural PostgreSQL time expiry permits a new publisher to reclaim the receipt;
- the old publisher is fenced with `ClaimLost`;
- source redelivery preserves offset, deterministic delivery UUID, and exact malformed bytes;
- both attempts use the same deterministic DLQ broker message ID;
- enabled broker deduplication produces `0 -> 1 -> 1` physical counts;
- disabled broker deduplication produces `0 -> 1 -> 2` physical counts;
- `published` precedes source acknowledgement;
- `acknowledged` remains bookkeeping after the source commit;
- the source cursor advances to the next offset only after the terminal result is durable.

## Non-claims

This source does not claim:

- a PostgreSQL/Iggy transaction;
- physical exactly-once delivery without server-side deduplication;
- that any production dedup expiry/capacity window is sufficient for every outage;
- active server-configuration readback;
- bundled mode, TLS, authentication coverage, or failover;
- multi-replica ownership proof;
- any Profiles authorization decision.

A production dedup window must remain large enough to contain the actual publish-to-recovery interval and must avoid capacity eviction of the deterministic message ID. Those deployment properties require separate reviewed configuration and retained runtime evidence.

No tests, Cargo commands, formatters, verifiers, PostgreSQL scenarios, or Iggy scenarios were run while defining this source contract.
