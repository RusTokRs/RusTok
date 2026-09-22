# Social Graph Index raw poison PostgreSQL/Iggy evidence

## Purpose

`index_raw_poison_postgres_iggy.rs` is an opt-in source harness for the first approved undecodable Social Graph Index consumer path. It composes a real external Iggy cursor with the production neutral `ConsumerPoisonReceiptStore` on PostgreSQL.

The harness is intentionally narrower than the server worker. It proves the persistence and broker ordering with public production APIs while `verify-social-graph-index-raw-poison-postgres-iggy.mjs` locks parity with `apps/server/src/services/social_graph_index_worker.rs`.

## Required services

Provide an operator-approved PostgreSQL database and a disposable external Iggy broker:

```text
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_DATABASE_URL=postgresql://...
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_ADDRESS=host:8090
```

Optional Iggy credentials must be supplied together:

```text
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_USERNAME=...
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_PASSWORD=...
```

There is no `DATABASE_URL`, localhost, or default-credential fallback. Missing required variables make a direct developer invocation report a skip; a future retained runner must reject that skip.

This source slice is external TCP/non-TLS only. TLS, authentication failure, failover, bundled mode, and multi-replica behavior remain separate evidence.

## Isolation

Each case creates:

- one unique PostgreSQL schema;
- one connection per SeaORM pool so session-local `search_path` remains deterministic;
- only the connector receipt migrations inside that schema;
- one unique Iggy stream;
- one `domain` partition and matching `dlq` partition;
- one unique DLQ observer group.

Cleanup drops only the unique PostgreSQL schema. The harness does not call an unreviewed Iggy stream deletion API, so use a disposable broker or an operator-approved cleanup process.

## Case 1: published before source acknowledgement

The first case publishes two distinct malformed source payloads and requires this order:

1. Open the fixed production source group and unique DLQ group before fixture publication.
2. Receive the first payload as `ConsumedContractDecodeFailure` without source acknowledgement.
3. Build the neutral connector identity and obtain one `Claimed` publication lease.
4. Observe receipt state `publishing`.
5. Publish exact bytes through `IggyTransport::move_to_dlq`.
6. Receive and acknowledge the physical DLQ payload through a real connector cursor.
7. Re-read PostgreSQL and require the receipt to remain `publishing`; broker success alone must not invent durable `published` state.
8. Call `mark_published` and require state `published`.
9. Acknowledge the exact source decode failure.
10. Re-read PostgreSQL and require the receipt to remain `published` until bookkeeping runs.
11. Call `mark_acknowledged` and require state `acknowledged`.
12. Receive the second malformed source payload at a greater offset.

This proves the intended boundary:

```text
reserve/claim -> exact broker publish -> durable published -> source ack -> acknowledged bookkeeping
```

## Case 2: acknowledgement-only redelivery

The second case persists `published` but deliberately shuts down the first transport without source acknowledgement.

After reopening the same unique stream with the fixed Social Graph Index consumer group, it requires:

- the same source offset;
- the same exact bytes;
- the same deterministic connector delivery UUID;
- `reserve_and_claim` to return `AlreadyPublished`;
- no second `move_to_dlq` call after reopen;
- source acknowledgement followed by `mark_acknowledged`;
- no second physical DLQ message during the bounded observation window;
- the next source payload at a greater offset.

The bounded absence check is supporting evidence only. The source verifier also requires that the recovery portion contains no DLQ publication call and that the production worker recognizes `AlreadyPublished`/`AlreadyAcknowledged` before acknowledgement-only recovery.

## Production parity

The verifier requires the production worker to retain this ordering:

```text
reserve_and_claim
transport.move_to_dlq
mark_raw_poison_published
acknowledge_decode_failure
mark_acknowledged
```

It also requires terminal receipt recognition to skip publication and retry source acknowledgement only.

The harness does not replace the server worker and does not create a second production policy implementation. Fixture publication is the only low-level injection path; receive, decode failure, DLQ publication, receipt transitions, and source acknowledgement use public production APIs.

## Non-claims

Even after successful execution, this evidence does not prove:

- a transaction between PostgreSQL and Iggy;
- physical exactly-once publication;
- that an Iggy deduplication window covers the full production recovery horizon;
- crash behavior in the broker-success/`mark_published` ambiguity window;
- concurrent multi-replica claim ownership in this combined scenario;
- bundled mode, TLS, authentication failure, or failover;
- Profiles visibility or authorization.

Profiles continues to authorize only through owner policy and authoritative Social Graph ports. Broker, receipt, metric, and evidence state never authorizes presentation.

## Maintainer commands

```bash
node scripts/verify/verify-social-graph-index-raw-poison-postgres-iggy.mjs

RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_DATABASE_URL='postgresql://...' \
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_ADDRESS='host:8090' \
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_USERNAME='...' \
RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_PASSWORD='...' \
  cargo test -p rustok-social-graph --features index-consumer \
  --test index_raw_poison_postgres_iggy -- --nocapture --test-threads=1
```

The username/password variables may both be omitted when the disposable broker permits anonymous access.

## Evidence status

The integration source, machine contract, production-order verifier, and this operator guide are source-complete. `execution_status` remains `not_run`. No Cargo command, source verifier, PostgreSQL query, external Iggy scenario, formatter, or multi-replica scenario was executed while authoring this slice.
