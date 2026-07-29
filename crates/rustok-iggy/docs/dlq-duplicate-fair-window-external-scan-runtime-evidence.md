# External-Iggy fair-window duplicate scan runtime evidence

Status: **two-partition harness and retained-capture tooling source complete; runtime execution and canonical packet pending**.

## Purpose

This evidence slice proves the production-reachable difference between the bounded
`fair_window` policy and the compatibility `global_budget` request across two
physical DLQ partitions. It also defines a clean-commit runner and a strict
privacy-safe retained packet verifier.

It does not prove moving cursors, complete history, deduplication-window
sufficiency, or that one deterministic broker message ID can be split across
partitions.

## Exact case

```text
fair_window_scans_each_partition_and_differs_from_global_budget
```

Target:

```text
crates/rustok-iggy/tests/dlq_duplicate_fair_window_external_scan.rs
```

Exact Cargo command:

```bash
cargo test -p rustok-iggy --features iggy \
  --test dlq_duplicate_fair_window_external_scan -- \
  fair_window_scans_each_partition_and_differs_from_global_budget \
  --exact --nocapture --test-threads=1
```

The source harness reads:

```text
RUSTOK_IGGY_FAIR_WINDOW_SCAN_TEST_ADDRESS
RUSTOK_IGGY_FAIR_WINDOW_SCAN_TEST_USERNAME       optional
RUSTOK_IGGY_FAIR_WINDOW_SCAN_TEST_PASSWORD       optional
```

The address has no default. Missing configuration skips the source harness, so
the retained runner rejects skip output and requires exactly one passing test.

## Reviewed broker and configuration

The broker must be disposable or operator-cleaned and must have Iggy message-ID
deduplication disabled. The test does not read server configuration.

The retained runner additionally requires:

```text
RUSTOK_IGGY_FAIR_WINDOW_SCAN_TEST_CONFIG_PATH
RUSTOK_IGGY_FAIR_WINDOW_SCAN_TEST_SERVER_ARTIFACT
```

`CONFIG_PATH` must be an absolute regular file outside the repository. The
runner reads only `[system.message_deduplication].enabled`, requires `false`,
and retains only:

```text
section
enabled
canonical_sha256
```

The full configuration, its path, and its full-file digest are not retained.
`SERVER_ARTIFACT` is a bounded operator-reviewed version or digest label, not an
endpoint.

## Production partition invariant

`IggyTransport::move_to_dlq` uses `IggyDlqPublisher` for deterministic broker
message IDs. The publisher selects:

```text
partition = (broker_message_id_as_u128 mod partition_count) + 1
```

Every physical copy with the same broker message ID is therefore colocated in
one partition. The harness preserves this production invariant and uses no
direct SDK fixture producer.

## Fixture

Five messages are published only through production
`IggyTransport::move_to_dlq`:

```text
partition 1:
  A1, A2  same deterministic UUID and same exact bytes
  C       one additional unique message beyond the fair cap

partition 2:
  B1, B2  same deterministic UUID and different exact bytes
```

The fixed non-nil UUIDs are checked against the production modulo rule before
publication.

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

This proves that partition 1 cannot consume partition 2's budget.

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

The global result must differ from the fair result.

## Offset non-mutation

The standalone scanner consumer offset must be absent for both configured
partitions:

```text
before fixture publication
after first fair scan
after compatibility global scan
after second fair scan
```

The retained packet stores only the aggregate assertion that two partitions
were checked and zero stored offsets were present at every checkpoint. It does
not retain partition IDs or offset values.

## Retained capture

Machine contract:

```text
crates/rustok-iggy/contracts/evidence/
  dlq-duplicate-fair-window-external-scan-execution-contract.json
```

Runner:

```text
scripts/evidence/
  capture-iggy-dlq-duplicate-fair-window-external-scan.mjs
```

Verifier:

```text
scripts/verify/
  verify-iggy-dlq-duplicate-fair-window-external-scan-retained.mjs
```

Canonical packet path:

```text
crates/rustok-iggy/contracts/evidence/
  dlq-duplicate-fair-window-external-scan-execution.json
```

The runner requires a clean worktree and a full unchanged commit, hashes every
bound source before and after the exact test, rejects skip output, and records
bounded Cargo/Rust/Iggy labels plus the test-output digest and byte count.

Packet publication is no-clobber. A temporary file is created with exclusive
creation and hard-linked to the canonical path. An existing canonical packet is
never replaced.

Example capture:

```bash
RUSTOK_IGGY_FAIR_WINDOW_SCAN_TEST_ADDRESS='host:8090' \
RUSTOK_IGGY_FAIR_WINDOW_SCAN_TEST_CONFIG_PATH='/outside/repo/iggy.toml' \
RUSTOK_IGGY_FAIR_WINDOW_SCAN_TEST_SERVER_ARTIFACT='iggy-server-reviewed-build' \
node scripts/evidence/capture-iggy-dlq-duplicate-fair-window-external-scan.mjs
```

Source and retained guards:

```bash
node scripts/verify/verify-iggy-dlq-duplicate-fair-window-external-scan-runtime.mjs
node scripts/verify/verify-iggy-dlq-duplicate-fair-window-external-scan-retained.mjs
```

Before execution, the retained verifier succeeds only when the execution
contract and runner are locked and the canonical packet is absent. After
execution it additionally requires the packet commit and all source hashes to
match the current checkout.

## Privacy boundary

The retained packet excludes:

```text
broker address
configuration path or full content
username/password/connection string
raw test output
stream name
partition IDs
offsets
broker/delivery UUIDs
payloads and payload digests
ack tokens
raw Iggy errors
```

It retains only identifier-free fair/global summaries, aggregate absent-offset
assertions, reviewed configuration projection, bounded artifact/toolchain
labels, current source hashes, timestamps, and output digest/size.

## Non-claims

This source slice does not claim:

- runtime execution or a canonical retained packet;
- active server configuration readback;
- production history completeness or deduplication-window sufficiency;
- same-ID cross-partition publication;
- moving cursors or cross-cycle duplicate accumulation;
- bundled Iggy, TLS/auth/failover, destructive reconciliation, or Profiles policy.

No test, verifier, Cargo command, broker connection, or retained capture was run
by the implementation agent.
