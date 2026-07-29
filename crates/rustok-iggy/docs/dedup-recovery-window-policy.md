# Iggy deduplication recovery-window policy

Status: **policy and retained-calibration tooling source complete; runtime calibration and canonical packet pending**.

## Purpose

The short external-Iggy behavior cases prove that message-ID deduplication can be disabled, can suppress an immediate repeated ID, and can lose suppression through capacity eviction or expiry. They do not establish that one reviewed production configuration covers the complete interval between a successful DLQ publish and the latest supported recovery attempt.

`IggyDedupRecoveryWindowPolicy` provides a transport-neutral, identifier-free comparison for that missing decision. It does not connect to Iggy, read active server state, mutate receipts, publish messages, commit offsets, or authorize Profiles.

## Required model

The caller supplies bounded upper limits for:

```text
publication lease
process restart
transport reconnect
operator recovery
```

The required expiry is the checked sum of those four intervals. Overflow fails closed.

The caller also supplies the maximum number of distinct deterministic message IDs that may be routed to one physical DLQ partition during the same interval. The configured `max_entries` must not be below that per physical partition bound.

No production default is embedded in the crate. The lease, restart, reconnect, operator-response, and per-partition workload limits must come from reviewed deployment and operating policy.

## Reviewed configuration input

The policy accepts only one of these explicit states:

```text
disabled
enabled(max_entries > 0, expiry > 0)
```

A disabled configuration never reports sufficient. Invalid enabled values fail closed before assessment.

## Assessment statuses

The identifier-free result distinguishes:

```text
disabled
insufficient_expiry
insufficient_capacity
insufficient_expiry_and_capacity
sufficient
```

`Sufficient` means only that the reviewed `expiry` and `max_entries` cover the supplied bounded model. It does not prove exactly-once, a PostgreSQL/Iggy transaction, active configuration readback, workload-bound correctness, failover behavior, or multi-replica behavior.

## Source contract

```text
crates/rustok-iggy/contracts/evidence/dedup-recovery-window-policy-source.json
```

Static verifier:

```bash
node scripts/verify/verify-iggy-dedup-recovery-window-policy.mjs
```

Focused source tests cover invalid policy/configuration, duration overflow, disabled mode, independent expiry/capacity deficits, and exact-boundary sufficiency.

## Retained calibration

The retained path requires two operator-reviewed files outside the repository.

Recovery bounds JSON:

```json
{
  "schema_version": 1,
  "publication_lease_milliseconds": 30000,
  "process_restart_milliseconds": 20000,
  "transport_reconnect_milliseconds": 10000,
  "operator_recovery_milliseconds": 60000,
  "required_max_entries_per_partition": 500,
  "capacity_basis": "reviewed peak per-partition arrival bound"
}
```

Iggy configuration:

```toml
[system.message_deduplication]
enabled = true
max_entries = 500
expiry = "2m"
```

The capture reads only the versioned bounds allowlist and the Iggy deduplication section. It retains canonical projections and their SHA-256 values, not file paths, full contents, or full-file hashes.

Execution contract:

```text
crates/rustok-iggy/contracts/evidence/
  dedup-recovery-window-calibration-execution-contract.json
```

Exact environment-driven Rust case:

```text
crates/rustok-iggy/tests/dedup_recovery_window_calibration.rs
reviewed_configuration_covers_recovery_window
```

Capture and retained verifier:

```bash
RUSTOK_IGGY_DEDUP_RECOVERY_BOUNDS_PATH=/outside/repository/bounds.json \
RUSTOK_IGGY_DEDUP_RECOVERY_CONFIG_PATH=/outside/repository/iggy.toml \
RUSTOK_IGGY_DEDUP_RECOVERY_SERVER_ARTIFACT=reviewed-iggy-build \
node scripts/evidence/capture-iggy-dedup-recovery-window-calibration.mjs

node scripts/verify/verify-iggy-dedup-recovery-window-retained.mjs
```

The runner requires one clean unchanged commit, exact one-test success, no skip, current source hashes, a reviewed server artifact label, sufficient expiry and capacity, and a clean worktree after execution. Publication is no-clobber.

The canonical packet remains pending until a maintainer supplies reviewed production inputs and runs the capture. Any bound source, configuration, or recovery-bounds change makes an existing packet stale.

## Remaining evidence

Before a stronger duplicate-suppression statement is made, operators still need to:

1. review the maximum lease, restart, reconnect, and intervention bounds;
2. review the maximum distinct-ID arrival bound per physical partition;
3. execute the clean-commit retained calibration and commit the no-clobber packet;
4. repeat calibration whenever a bound source, configuration, or input changes;
5. separately prove bundled mode, TLS/auth/failover, capacity pressure, and multi-replica recovery.

The policy and packet remain operational evidence only. Profiles privacy and visibility continue to resolve exclusively through authoritative owner ports.
