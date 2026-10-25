# External Iggy physical DLQ header evidence

## Purpose

`contract_poison_external_iggy_header.rs` is an opt-in source harness for one narrow runtime claim: a deterministic connector delivery UUID attached to a raw contract poison `DlqEntry` is physically written as the Iggy message header ID, and the same UUID selects the expected one-based DLQ partition.

This is intentionally separate from `contract_poison_external_iggy.rs`:

- the lifecycle harness proves production source receive, no-ack transport reconnect/redelivery, exact-byte DLQ, and explicit source cursor advancement;
- the header harness proves one physical DLQ message's header ID, partition, and payload;
- future deduplication scenarios must remain separate because a repeated publication can be accepted, suppressed, expired, or evicted depending on broker configuration.

## Production and probe boundaries

The production path is:

1. construct `ConsumedContractDecodeFailure` with the production validated type;
2. call `to_dlq_entry(1)`;
3. read the explicit `broker_message_id` from that entry;
4. publish exactly once through `IggyTransport::move_to_dlq`;
5. shut down through `IggyTransport::shutdown`.

The direct Iggy SDK is a probe only. It may:

- connect to the reviewed external endpoint;
- open one unique consumer group on the unique stream's `dlq` topic before publication;
- receive one message;
- read `message.header.id`, `partition_id`, and payload;
- commit that probe message's physical header offset.

The SDK probe must not publish, create a source fixture, acknowledge a production source cursor, modify connector receipts, delete a stream, or change broker deduplication.

## Broker selection

Set:

```text
RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS=host:8090
```

Optional credentials must be supplied together:

```text
RUSTOK_IGGY_EXTERNAL_TEST_USERNAME=...
RUSTOK_IGGY_EXTERNAL_TEST_PASSWORD=...
```

There is no localhost or default-credential fallback. The address is `host:port` without scheme, embedded credentials, or query parameters. This slice is TCP/non-TLS only.

Every run creates a unique three-partition stream and a unique DLQ probe group. Use a disposable external Iggy server or an operator-approved cleanup process; the harness does not call an unreviewed stream deletion API.

## Assertions

The harness creates one non-empty synthetic decode failure with stable source metadata and derives its production DLQ entry.

For `N = 3`, the expected partition is:

```text
(uuid_as_u128 mod N) + 1
```

The source test requires this value to be in `1..=N`, then publishes one entry. The physical SDK message must satisfy all of the following:

- `message.header.id == delivery_uuid.as_u128()`;
- `partition_id == expected_partition`;
- physical payload bytes equal the exact DLQ payload;
- the probe commits `message.header.offset` for that same partition.

The SDK polling cursor's `current_offset` is not treated as the physical delivery offset and is not compared to the message header offset.

## Non-claims

Even after successful execution, this harness does not prove:

- source consumer receive, redelivery, or acknowledgement lifecycle;
- PostgreSQL receipt ordering or durable `published` state;
- duplicate suppression with deduplication disabled or enabled;
- deduplication capacity or expiry windows;
- physical exactly-once publication;
- bundled mode;
- TLS/authentication/failover behavior;
- multi-replica behavior;
- Profiles privacy or authorization.

## Maintainer commands

```bash
RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS='iggy.example:8090' \
RUSTOK_IGGY_EXTERNAL_TEST_USERNAME='...' \
RUSTOK_IGGY_EXTERNAL_TEST_PASSWORD='...' \
  cargo test -p rustok-iggy --features iggy \
  --test contract_poison_external_iggy_header -- --nocapture --test-threads=1

node scripts/verify/verify-iggy-contract-poison-external-header-evidence.mjs
```

Username/password may both be omitted for a disposable anonymous broker. Never provide only one.

## Evidence status

The test, versioned source contract, probe boundary, and static verifier are source-complete. `execution_status` remains `not_run`. No Cargo command, source verifier, external Iggy scenario, or broker configuration change was executed while authoring this slice.
