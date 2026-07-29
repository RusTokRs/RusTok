# Iggy deduplication recovery-window policy

Status: **source complete; runtime calibration remains pending**.

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

## Remaining evidence

Before a stronger duplicate-suppression statement is made, operators still need to:

1. publish reviewed maximum lease, restart, reconnect, and intervention bounds;
2. derive a defensible maximum distinct-ID arrival bound per physical partition;
3. bind the assessment to a reviewed Iggy configuration projection and digest;
4. retain the assessment from one clean commit;
5. repeat evidence for bundled mode, TLS/auth/failover, capacity pressure, and multi-replica recovery.

The policy remains operational evidence only. Profiles privacy and visibility continue to resolve exclusively through authoritative owner ports.
