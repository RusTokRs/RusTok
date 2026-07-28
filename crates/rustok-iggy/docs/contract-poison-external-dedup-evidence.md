# External Iggy message-ID deduplication evidence

## Purpose

`contract_poison_external_iggy_dedup.rs` defines four opt-in behavior scenarios for Iggy's server-side message-ID deduplication. The production publisher is always `IggyTransport::move_to_dlq` with a deterministic raw-poison `broker_message_id`. A separate SDK observer reads only partition `messages_count` from the unique stream's `dlq` topic.

The test does not read or mutate Iggy server configuration. A retained execution therefore pairs each run with an externally reviewed configuration file and a bounded server artifact/version label. The capture runner extracts only `[system.message_deduplication]`, validates the expected mode, canonicalizes its non-secret values, and stores a SHA-256 of that canonical section. It never persists the full configuration file, its path, or its full-file digest.

## Required disposable brokers

Provide four distinct external Iggy addresses backed by separately reviewed disposable configurations. The retained runner rejects duplicate addresses and duplicate config-file paths.

### Disabled

```toml
[system.message_deduplication]
enabled = false
```

Required inputs:

```text
RUSTOK_IGGY_DEDUP_DISABLED_ADDRESS=host:port
RUSTOK_IGGY_DEDUP_DISABLED_CONFIG_PATH=/outside/repository/disabled.toml
RUSTOK_IGGY_DEDUP_DISABLED_SERVER_ARTIFACT=iggy-server-0.10.0-or-reviewed-image-digest
```

Publishing immutable entry `A` twice must produce partition counts:

```text
0 -> 1 -> 2
```

`max_entries` and `expiry` may be omitted in the disabled configuration. When present, they are parsed and retained in the canonical non-secret section.

### Enabled, entry retained

```toml
[system.message_deduplication]
enabled = true
max_entries = 1000
expiry = "10s"
```

Required inputs:

```text
RUSTOK_IGGY_DEDUP_ENABLED_ADDRESS=host:port
RUSTOK_IGGY_DEDUP_ENABLED_CONFIG_PATH=/outside/repository/enabled.toml
RUSTOK_IGGY_DEDUP_ENABLED_SERVER_ARTIFACT=iggy-server-0.10.0-or-reviewed-image-digest
```

`max_entries` must be at least `1`; `expiry` must be a positive duration. Publishing `A` twice immediately must produce:

```text
0 -> 1 -> 1
```

### Capacity eviction

```toml
[system.message_deduplication]
enabled = true
max_entries = 1
expiry = "10s"
```

Required inputs:

```text
RUSTOK_IGGY_DEDUP_CAPACITY_ADDRESS=host:port
RUSTOK_IGGY_DEDUP_CAPACITY_CONFIG_PATH=/outside/repository/capacity.toml
RUSTOK_IGGY_DEDUP_CAPACITY_SERVER_ARTIFACT=iggy-server-0.10.0-or-reviewed-image-digest
```

The scenario publishes `A`, repeats `A`, publishes distinct `B`, then publishes `A` again:

```text
0 -> 1 -> 1 -> 2 -> 3
```

The immediate repeated `A` establishes suppression before `B` is introduced. Acceptance of `A` after `B` is the observed capacity-eviction behavior for the reviewed one-entry per-partition configuration.

### Expiry

```toml
[system.message_deduplication]
enabled = true
max_entries = 1000
expiry = "1s"
```

Required inputs:

```text
RUSTOK_IGGY_DEDUP_EXPIRY_ADDRESS=host:port
RUSTOK_IGGY_DEDUP_EXPIRY_CONFIG_PATH=/outside/repository/expiry.toml
RUSTOK_IGGY_DEDUP_EXPIRY_SERVER_ARTIFACT=iggy-server-0.10.0-or-reviewed-image-digest
RUSTOK_IGGY_DEDUP_EXPIRY_WAIT_MS=<100..300000>
```

The runner supports positive TOML duration strings with units `ms`, `s`, `m`, `h`, or `d` and requires the reviewed expiry to be strictly shorter than the configured wait. The scenario publishes `A`, repeats it immediately, waits, then publishes `A` again:

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
- deterministic UUIDs derived through `ConsumedContractDecodeFailure::to_dlq_entry`;
- explicit observer and production transport shutdown.

There are no default addresses or credentials. Use disposable brokers or an operator-approved cleanup process; neither the test nor the retained runner deletes streams.

## Reviewed configuration boundary

Each `*_CONFIG_PATH` must point to an existing file outside the repository. The runner locates exactly one `[system.message_deduplication]` section, parses `enabled`, `max_entries`, and `expiry`, and rejects duplicate keys or a mismatched scenario configuration.

Only this canonical object is retained:

```json
{
  "section": "system.message_deduplication",
  "enabled": true,
  "max_entries": 1,
  "expiry": "10s",
  "expiry_milliseconds": 10000,
  "canonical_sha256": "..."
}
```

The packet does not contain:

- broker addresses;
- configuration paths;
- full configuration contents or full-file digests;
- usernames, passwords, or connection strings;
- raw test output;
- delivery UUIDs, payloads, or source coordinates.

The `server_artifact` value is an operator-reviewed bounded version/image identifier supplied through the scenario-specific `*_SERVER_ARTIFACT` environment variable. It is not server readback.

## Observation boundary

The SDK observer may only:

- connect;
- call `get_topic` for the unique stream's `dlq` topic;
- read partition `1` and its `messages_count`;
- shut down.

It does not consume payloads, publish, store offsets, modify configuration, delete streams, or mutate connector receipts.

The test reads counts immediately after each successful production publish. It does not use an absence timeout to decide whether a duplicate was suppressed. The single sleep is bounded and belongs only to the expiry scenario.

## Retained execution

`contract-poison-external-iggy-dedup-execution-contract.json` locks:

- the four required scenario/address/config/artifact environment sets;
- exact per-case Cargo command construction using a libtest `--exact` filter;
- expected count sequences;
- reviewed configuration fields and privacy boundary;
- production source files whose SHA-256 values bind the packet;
- the canonical evidence path.

`capture-iggy-contract-poison-external-dedup.mjs` requires:

1. a clean Git working tree and full commit SHA;
2. all four distinct addresses;
3. all four distinct external config files;
4. all four bounded server artifact labels;
5. valid optional credential pairing;
6. a bounded expiry wait longer than the reviewed expiry;
7. one exact named test per Cargo invocation;
8. `running 1 test` and `<case> ... ok` for each invocation;
9. no skip output;
10. unchanged commit, source hashes, and clean working tree after all commands.

Only after all checks pass does it atomically write:

```text
crates/rustok-iggy/contracts/evidence/contract-poison-external-iggy-dedup-execution.json
```

The packet contains bounded toolchain/timestamp metadata, source hashes, canonical reviewed configuration digests, server artifact labels, exact command arrays, expected count sequences, and per-case/combined output hashes and byte counts.

`verify-iggy-contract-poison-external-dedup-retained-evidence.mjs` verifies the source contract while the packet is absent. Once a packet exists, it additionally requires current source hashes, exact fields and commands, valid canonical configuration digests, four all-pass cases, bounded metadata, and no forbidden persisted fields.

## Non-claims

Even after successful execution, these scenarios do not prove:

- that the active server configuration was read back through Iggy;
- physical exactly-once publication;
- a transaction between PostgreSQL receipt state and Iggy publication;
- that the selected `max_entries`/`expiry` window covers the maximum production lease, restart, reconnect, and recovery horizon;
- bundled mode;
- TLS/authentication/failover behavior;
- multi-replica behavior;
- Profiles privacy or authorization.

Production confirmation policy still requires reviewed recovery-window evidence, or a stronger database-owned outbox/broker transaction design.

## Maintainer commands

Source-only checks:

```bash
node scripts/verify/verify-iggy-contract-poison-external-dedup-evidence.mjs
node scripts/verify/verify-iggy-contract-poison-external-dedup-retained-evidence.mjs
```

Create the retained packet from a clean commit:

```bash
RUSTOK_IGGY_DEDUP_DISABLED_ADDRESS='disabled.example:8090' \
RUSTOK_IGGY_DEDUP_DISABLED_CONFIG_PATH='/secure/reviewed/disabled.toml' \
RUSTOK_IGGY_DEDUP_DISABLED_SERVER_ARTIFACT='iggy-server-0.10.0' \
RUSTOK_IGGY_DEDUP_ENABLED_ADDRESS='enabled.example:8090' \
RUSTOK_IGGY_DEDUP_ENABLED_CONFIG_PATH='/secure/reviewed/enabled.toml' \
RUSTOK_IGGY_DEDUP_ENABLED_SERVER_ARTIFACT='iggy-server-0.10.0' \
RUSTOK_IGGY_DEDUP_CAPACITY_ADDRESS='capacity.example:8090' \
RUSTOK_IGGY_DEDUP_CAPACITY_CONFIG_PATH='/secure/reviewed/capacity.toml' \
RUSTOK_IGGY_DEDUP_CAPACITY_SERVER_ARTIFACT='iggy-server-0.10.0' \
RUSTOK_IGGY_DEDUP_EXPIRY_ADDRESS='expiry.example:8090' \
RUSTOK_IGGY_DEDUP_EXPIRY_CONFIG_PATH='/secure/reviewed/expiry.toml' \
RUSTOK_IGGY_DEDUP_EXPIRY_SERVER_ARTIFACT='iggy-server-0.10.0' \
RUSTOK_IGGY_DEDUP_EXPIRY_WAIT_MS='1500' \
  node scripts/evidence/capture-iggy-contract-poison-external-dedup.mjs

node scripts/verify/verify-iggy-contract-poison-external-dedup-retained-evidence.mjs
```

Optional shared username/password may be added to the runner environment as a pair.

## Evidence status

The four behavior scenarios, versioned source and execution contracts, reviewed-config parser, clean-commit runner, read-only observer boundary, and static/persistent verifiers are source-complete. The canonical execution JSON is intentionally absent until a maintainer completes all four external-Iggy runs. No Cargo command, verifier, external Iggy scenario, server restart, or configuration change was performed while authoring this slice.
