# External Iggy raw contract poison evidence

## Scope

`contract_poison_external_iggy.rs` is an opt-in integration harness for the real external Iggy cursor and DLQ path. It proves the transport-level boundary around broker bytes that cannot decode into a trusted contract envelope.

The harness uses production APIs for:

- topology startup through `IggyTransport::new`;
- persistent typed receive through `open_persistent_contract_consumer_group` and `receive_delivery`;
- exact-byte raw failure conversion through `ConsumedContractDecodeFailure`;
- deterministic DLQ entry construction through `to_dlq_entry`;
- DLQ publication through `IggyTransport::move_to_dlq`;
- source commit through `acknowledge_decode_failure`;
- DLQ receive/commit through a real `ExternalConnector` consumer-group cursor.

A separate `ExternalConnector::publish(PublishRequest)` is fixture-only. It injects arbitrary malformed bytes because the typed event publisher correctly refuses to create an invalid contract envelope. It does not implement receive, decode, DLQ, or acknowledgement behavior.

## Broker selection

Set:

```text
RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS=host:8090
```

Optional credentials must be supplied as a pair:

```text
RUSTOK_IGGY_EXTERNAL_TEST_USERNAME=...
RUSTOK_IGGY_EXTERNAL_TEST_PASSWORD=...
```

The address contains no scheme, credentials, or query string. The harness has no localhost/default-credential fallback. If the address is absent, the direct test reports a skip and returns successfully.

This first slice is TCP/non-TLS only. TLS, authentication failures, address failover, and certificate validation remain separate runtime evidence.

## Isolation and cleanup

Every execution creates:

- one unique stream;
- one unique source consumer group;
- one unique DLQ consumer group;
- one `domain` partition and one matching `dlq` partition.

Use a disposable external Iggy server or an operator-approved cleanup process. The test intentionally does not call an unreviewed stream deletion API. A failed run may therefore leave a uniquely named stream that is safe to identify and remove later.

The test uses bounded receive timeouts but no sleeps. It opens both consumer groups before fixture publication so evidence does not depend on an implicit latest/earliest subscription default.

## Scenario

Two distinct non-empty malformed payloads are published to the single `domain` partition.

1. The production contract cursor receives the first payload as `DecodeFailure` without committing it.
2. Exact bytes, offset, ack token, stable `decode_invalid` classification, and deterministic connector delivery UUID are retained.
3. The failure is published to `dlq` through `IggyTransport::move_to_dlq`.
4. The independent DLQ cursor receives the exact first payload and explicitly acknowledges it.
5. The source cursor is dropped without source acknowledgement.
6. Reopening the same source group must redeliver the same offset, bytes, and delivery UUID.
7. Explicit source acknowledgement then permits the cursor to receive the second payload at a greater offset.
8. The second payload is also published to DLQ, verified byte-for-byte, and explicitly acknowledged on both DLQ and source cursors.
9. The fixture connector and transport shut down explicitly.

## Evidence boundary

This scenario proves only the real external broker cursor and transport path described above. It does **not** prove:

- connector database receipt ordering or `published` persistence;
- the deterministic UUID being present in the physical Iggy message header;
- broker duplicate suppression, cache capacity, or expiry;
- physical exactly-once publication;
- bundled mode;
- TLS/authentication/failover behavior;
- multi-replica ownership or rebalance behavior;
- Profiles privacy, which remains owner-port based and independent of broker evidence.

Those claims require separate retained evidence.

## Maintainer commands

```bash
RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS='iggy.example:8090' \
RUSTOK_IGGY_EXTERNAL_TEST_USERNAME='...' \
RUSTOK_IGGY_EXTERNAL_TEST_PASSWORD='...' \
  cargo test -p rustok-iggy --features iggy \
  --test contract_poison_external_iggy -- --nocapture --test-threads=1

node scripts/verify/verify-iggy-contract-poison-external-evidence.mjs
```

The username/password variables may both be omitted when the disposable broker permits anonymous access. Never pass only one of the pair.

## Evidence status

The source contract, opt-in harness, and static verifier are source-complete. The contract remains `source_complete_runtime_pending`, and no Cargo command, source verifier, or external Iggy scenario was executed while authoring this slice.
