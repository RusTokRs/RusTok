# Profiles checkpoint: Iggy deduplication recovery window

Status: **source-complete; runtime calibration and retained evidence pending**.

## Why this belongs in the Profiles improvement trail

Profiles privacy is owner-port policy, but Social Graph relationship events feed downstream Index behavior used by profile discovery. Raw poison handling therefore needs an explicit reliability boundary without turning broker state into profile authorization.

The existing external-Iggy cases show immediate suppression, disabled deduplication, capacity eviction, and expiry. Those short sequences do not prove that production `max_entries` and `expiry` cover the complete broker-success-to-recovery interval.

## Delivered source policy

`rustok-iggy` now publishes:

```text
IggyDeduplicationConfiguration
IggyDedupRecoveryWindowPolicy
IggyDedupRecoveryWindowAssessment
IggyDedupRecoveryWindowStatus
IggyDedupRecoveryWindowPolicyError
```

The policy requires caller-reviewed bounds for publication lease, process restart, transport reconnect, and operator recovery. It uses their checked sum as the required expiry. It also requires an explicit maximum number of distinct deterministic message IDs that may share one physical partition during that horizon.

The assessment distinguishes disabled, insufficient expiry, insufficient capacity, both insufficient, and sufficient states. Invalid values and duration overflow fail closed. There are no production defaults.

Machine contract:

```text
crates/rustok-iggy/contracts/evidence/dedup-recovery-window-policy-source.json
```

Static verifier:

```bash
node scripts/verify/verify-iggy-dedup-recovery-window-policy.mjs
```

Owner guide:

```text
crates/rustok-iggy/docs/dedup-recovery-window-policy.md
```

## Profiles boundary

Profiles never authorizes visibility, `followers_only`, follow controls, search inclusion, or presentation from:

- deduplication enabled/disabled state;
- `max_entries` or `expiry`;
- recovery-window assessment status;
- receipt state, broker counts, offsets, lag, or evidence packets.

Privacy remains evaluated before localized and Media-backed presentation. Restricted or unavailable public rows remain absent.

A `sufficient` assessment means only that one reviewed configuration covers the supplied bounded model. It does not prove active server configuration, a database/broker transaction, exactly-once delivery, failover, workload bounds, or multi-replica recovery.

## Remaining evidence

1. provide reviewed maximum lease, restart, reconnect, and operator-response bounds;
2. derive the maximum distinct deterministic IDs per physical partition during that interval;
3. bind those inputs to reviewed external and bundled Iggy configuration digests;
4. execute the source verifier and focused Rust tests;
5. retain a clean-commit calibration packet;
6. separately prove TLS/auth/failover, capacity pressure, and multi-replica claim ownership.

Tests, Cargo commands, source verifiers, broker connections, retained capture, and multi-replica scenarios were not run per maintainer instruction.
