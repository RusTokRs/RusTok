# Moving-window external-Iggy observer runtime evidence

Status: **source-complete; external execution pending**.

## Purpose

This harness exercises the Iggy observer used by the explicit server `moving_window` mode. It proves one bounded cross-cycle relationship without starting the full server process.

The production-reachable fixture uses one configured partition. Both physical copies carry the same non-nil deterministic broker message ID and the same exact payload, so they remain in the same production-selected partition.

```text
publish copy A
  -> moving cycle 1 sees one unique message
publish copy A again
  -> moving cycle 2 advances and rolling state reports one duplicate
empty moving cycle 3
  -> retained summary remains unchanged
replacement observer
  -> starts again from reviewed initial_offset and sees the first copy only
```

## Locked moving configuration

The exact retained case uses:

```text
partition_count = 1
initial_offset = 0
per_partition_messages = 1
batch_size = 1
rolling_max_cycles = 3
rolling_max_observations_per_cycle = 1
```

There is no production default claim. This fixture only proves the supplied bounded configuration.

## Reviewed inputs

The capture runner requires four external inputs:

```text
RUSTOK_IGGY_MOVING_OBSERVER_TEST_ADDRESS
RUSTOK_IGGY_MOVING_OBSERVER_TEST_CONFIG_PATH
RUSTOK_IGGY_MOVING_OBSERVER_TEST_RESET_REVIEW_PATH
RUSTOK_IGGY_MOVING_OBSERVER_TEST_SERVER_ARTIFACT
```

Username and password are optional but must be supplied together.

The reviewed Iggy TOML must be outside the repository and set:

```toml
[system.message_deduplication]
enabled = false
```

The reviewed reset file must be outside the repository and contain exactly:

```json
{
  "schema_version": 1,
  "initial_offset": 0,
  "acceptable_reset_frequency": "reviewed bounded label",
  "restart_continuity_required": false,
  "review_scope": "reviewed bounded label"
}
```

A deployment requiring restart continuity cannot use this packet as approval for process-local cursors; it needs a separately reviewed persistent cursor owner.

## Assertions

The first cycle reports one unique physical message. The second cycle reports two retained observations, one duplicate message, one duplicate group, and a maximum of two copies. The third empty cycle equals the second summary. A replacement observer equals the first summary because it starts from `initial_offset = 0` with empty rolling history.

Stored standalone-consumer offsets must remain absent at all five checkpoints. The test emits one bounded identifier-free runtime marker; no broker coordinates or message identity are retained.

## Capture

```bash
RUSTOK_IGGY_MOVING_OBSERVER_TEST_ADDRESS='host:8090' \
RUSTOK_IGGY_MOVING_OBSERVER_TEST_CONFIG_PATH=/outside/repository/iggy.toml \
RUSTOK_IGGY_MOVING_OBSERVER_TEST_RESET_REVIEW_PATH=/outside/repository/reset-review.json \
RUSTOK_IGGY_MOVING_OBSERVER_TEST_SERVER_ARTIFACT=reviewed-iggy-build \
node scripts/evidence/capture-iggy-dlq-duplicate-moving-window-external-observer.mjs
```

The runner requires a clean unchanged commit, current source hashes, exactly one passing test, no skip, the locked runtime marker, and a clean tree after execution. Publication is no-clobber.

```bash
node scripts/verify/verify-iggy-dlq-duplicate-moving-window-external-observer-runtime.mjs
node scripts/verify/verify-iggy-dlq-duplicate-moving-window-external-observer-retained.mjs
```

## Privacy boundary

The packet retains canonical reviewed configuration and reset projections, bounded artifact/toolchain labels, source hashes, timestamps, exact assertions, and test-output digest/size.

It excludes addresses, paths, credentials, connection strings, stream names, partition IDs, offsets, message IDs, payloads/digests, raw output, ack tokens, and raw Iggy errors.

## Non-claims

This evidence does not start the full server process, read back active server environment, prove persisted progress, prove restart-safe progress, establish current-tail or complete-history coverage, calibrate production retention, cover bundled/TLS/auth/failover, prove exactly-once, authorize destructive reconciliation, or authorize Profiles.
