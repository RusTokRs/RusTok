# External-Iggy fair-window duplicate scan runtime evidence

Status: **source complete; runtime execution and retained packet pending**.

## Purpose

This harness proves the production-reachable difference between the bounded
`fair_window` policy and the compatibility `global_budget` request across two
physical DLQ partitions.

It does not prove moving cursors, complete history, or that one deterministic
broker message ID can be split across partitions.

## Exact case

```text
fair_window_scans_each_partition_and_differs_from_global_budget
```

Target:

```text
crates/rustok-iggy/tests/dlq_duplicate_fair_window_external_scan.rs
```

Exact command:

```bash
RUSTOK_IGGY_FAIR_WINDOW_SCAN_TEST_ADDRESS='host:8090' \
cargo test -p rustok-iggy --features iggy \
  --test dlq_duplicate_fair_window_external_scan -- \
  fair_window_scans_each_partition_and_differs_from_global_budget \
  --exact --nocapture --test-threads=1
```

Optional credentials:

```text
RUSTOK_IGGY_FAIR_WINDOW_SCAN_TEST_USERNAME
RUSTOK_IGGY_FAIR_WINDOW_SCAN_TEST_PASSWORD
```

The address has no default. Missing configuration skips the source harness, so a
future retained runner must reject skips and require exactly one passing case.

## Reviewed broker

The broker must be disposable or operator-cleaned and must have Iggy message-ID
deduplication disabled. The test does not read server configuration.

The harness creates one unique stream with two domain/DLQ partitions and does not
delete it.

## Production partition invariant

`IggyTransport::move_to_dlq` uses `IggyDlqPublisher` when a deterministic broker
message ID is present. The publisher selects:

```text
partition = (broker_message_id_as_u128 mod partition_count) + 1
```

Therefore every physical copy with the same broker message ID is colocated in the
same partition. The harness deliberately preserves that production invariant and
does not use a direct SDK fixture producer to force an impossible split.

The scanner still combines observations from every requested partition before
classification. This matters for one aggregate summary, but production runtime
evidence must not claim that identical deterministic IDs were physically split
across partitions.

## Fixture

Five messages are published only through production
`IggyTransport::move_to_dlq`:

```text
partition 1:
  A1, A2  same deterministic header UUID and same exact bytes
  C       one additional unique message beyond the fair cap

partition 2:
  B1, B2  same deterministic header UUID and different exact bytes
```

The broker IDs are selected locally so the production modulo rule routes them to
the intended partition. IDs, payloads, stream names, offsets, and addresses are
never logged or retained.

## Fair-window proof

Policy:

```text
partitions = [1, 2]
start_offset = 0
per_partition_messages = 2
batch_size = 2
```

The first and second fair scans must both return:

```text
total_messages = 4
unique_message_ids = 2
duplicate_messages = 2
duplicate_groups = 2
conflicting_payload_groups = 1
max_copies_per_message_id = 2
has_physical_duplicates = true
has_identity_conflicts = true
requires_manual_review = true
```

This proves that partition 1 cannot consume partition 2's budget: the third
partition-1 message is outside the fair window while both partition-2 messages are
included.

## Compatibility-global comparison

Request:

```text
partitions = [1, 2]
start_offset = 0
max_messages = 4
batch_size = 2
```

The ordered global request must return:

```text
total_messages = 4
unique_message_ids = 3
duplicate_messages = 1
duplicate_groups = 1
conflicting_payload_groups = 0
max_copies_per_message_id = 2
has_physical_duplicates = true
has_identity_conflicts = false
requires_manual_review = false
```

The result must differ from the fair summary. Partition 1 consumes three of the
four global slots before partition 2 receives one.

## Offset non-mutation

The standalone scanner consumer offset must be absent for partitions 1 and 2:

```text
before fixture publication
after first fair scan
after compatibility global scan
after second fair scan
```

The harness contains no offset store/delete, consumer group, acknowledgement,
message send through the SDK observer, stream/topic deletion, or purge.

## Privacy and non-claims

Only identifier-free aggregate summaries are asserted. The harness does not
retain or print connection details, stream names, partitions, offsets, UUIDs,
payloads, payload digests, credentials, or raw Iggy errors.

It does not claim:

- runtime execution or retained evidence;
- active server configuration readback;
- production history completeness;
- deduplication-window sufficiency;
- same-ID cross-partition publication;
- moving cursors or cross-cycle duplicate accumulation;
- bundled Iggy, TLS/auth/failover, destructive reconciliation, or Profiles policy.

## Source guard

```bash
node scripts/verify/verify-iggy-dlq-duplicate-fair-window-external-scan-runtime.mjs
```

No test, verifier, Cargo command, broker connection, or retained capture was run
by the implementation agent.
