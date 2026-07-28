# External Iggy message-ID deduplication evidence

## Purpose

`contract_poison_external_iggy_dedup.rs` defines four opt-in behavior scenarios for Iggy's server-side message-ID deduplication. The production publisher is always `IggyTransport::move_to_dlq` with a deterministic raw-poison `broker_message_id`. A separate SDK observer reads only partition `messages_count` from the unique stream's `dlq` topic.

The test does not read or mutate Iggy server configuration. A retained execution must therefore pair each run with a reviewed server configuration artifact and must not infer configuration values from test code alone.

## Required disposable brokers

Provide four separately configured external Iggy instances or four independently restarted disposable configurations.

### Disabled

```toml
[system.message_deduplication]
enabled = false
```

Address:

```text
RUSTOK_IGGY_DEDUP_DISABLED_ADDRESS=host:port
```

Publishing the same immutable entry `A` twice must produce partition counts:

```text
0 -> 1 -> 2
```

### Enabled, entry retained

```toml
[system.message_deduplication]
enabled = true
max_entries = <at least 1>
expiry = <longer than the scenario>
```

Address:

```text
RUSTOK_IGGY_DEDUP_ENABLED_ADDRESS=host:port
```

Publishing `A` twice immediately must produce:

```text
0 -> 1 -> 1
```

### Capacity eviction

```toml
[system.message_deduplication]
enabled = true
max_entries = 1
expiry = <longer than the scenario>
```

Address:

```text
RUSTOK_IGGY_DEDUP_CAPACITY_ADDRESS=host:port
```

The scenario publishes `A`, repeats `A`, publishes distinct `B`, then publishes `A` again:

```text
0 -> 1 -> 1 -> 2 -> 3
```

The immediate repeated `A` proves suppression is active before `B` is introduced. Acceptance of `A` after `B` is the capacity-eviction behavior expected from a one-entry per-partition cache.

### Expiry

```toml
[system.message_deduplication]
enabled = true
max_entries = <at least 1>
expiry = <shorter than the configured test wait>
```

Address and bounded wait:

```text
RUSTOK_IGGY_DEDUP_EXPIRY_ADDRESS=host:port
RUSTOK_IGGY_DEDUP_EXPIRY_WAIT_MS=<100..300000>
```

The scenario publishes `A`, repeats `A` immediately, waits, then publishes `A` again:

```text
0 -> 1 -> 1 -> 2
```

The immediate repeat establishes active suppression; acceptance after the reviewed wait establishes observed expiry behavior for that execution.

## Shared credentials and transport

Optional shared credentials must be supplied as a pair:

```text
RUSTOK_IGGY_DEDUP_TEST_USERNAME=...
RUSTOK_IGGY_DEDUP_TEST_PASSWORD=...
```

Every scenario uses:

- external TCP mode;
- one unique stream;
- one `domain` partition and one matching `dlq` partition;
- replication factor `1`;
- exact deterministic UUIDs derived through `ConsumedContractDecodeFailure::to_dlq_entry`;
- explicit observer and production transport shutdown.

There are no default addresses or credentials. Use disposable brokers or an operator-approved cleanup process; the tests do not delete streams.

## Observation boundary

The SDK observer may only:

- connect;
- call `get_topic` for the unique stream's `dlq` topic;
- read partition `1` and its `messages_count`;
- shut down.

It does not consume payloads, publish, store offsets, modify configuration, delete streams, or mutate connector receipts.

The test reads counts immediately after each successful production publish. It does not use an absence timeout to decide whether a duplicate was suppressed. The single sleep is bounded and belongs only to the expiry scenario.

## Non-claims

Even after successful execution, these scenarios do not prove:

- that the test read back the active server configuration;
- physical exactly-once publication;
- a transaction between PostgreSQL receipt state and Iggy publication;
- that the selected `max_entries`/`expiry` window covers the maximum production lease, restart, reconnect, and recovery horizon;
- bundled mode;
- TLS/authentication/failover behavior;
- multi-replica behavior;
- Profiles privacy or authorization.

Production confirmation policy still requires reviewed retained configuration and recovery-window evidence, or a stronger database-owned outbox/broker transaction design.

## Maintainer command

Provide all four addresses to run the complete target:

```bash
RUSTOK_IGGY_DEDUP_DISABLED_ADDRESS='disabled.example:8090' \
RUSTOK_IGGY_DEDUP_ENABLED_ADDRESS='enabled.example:8090' \
RUSTOK_IGGY_DEDUP_CAPACITY_ADDRESS='capacity.example:8090' \
RUSTOK_IGGY_DEDUP_EXPIRY_ADDRESS='expiry.example:8090' \
RUSTOK_IGGY_DEDUP_EXPIRY_WAIT_MS='1500' \
RUSTOK_IGGY_DEDUP_TEST_USERNAME='...' \
RUSTOK_IGGY_DEDUP_TEST_PASSWORD='...' \
  cargo test -p rustok-iggy --features iggy \
  --test contract_poison_external_iggy_dedup -- --nocapture --test-threads=1

node scripts/verify/verify-iggy-contract-poison-external-dedup-evidence.mjs
```

A future retained runner must reject missing scenario addresses, record reviewed configuration digests without credentials, require all four named tests to execute rather than skip, and retain source/output hashes.

## Evidence status

The four behavior scenarios, versioned source contract, read-only observer boundary, and static verifier are source-complete. `execution_status` remains `not_run`. No Cargo command, verifier, external Iggy scenario, server restart, or configuration change was performed while authoring this slice.
