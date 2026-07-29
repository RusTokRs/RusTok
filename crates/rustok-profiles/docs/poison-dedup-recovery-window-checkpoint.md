# Profiles checkpoint: Iggy deduplication recovery window

Status: **policy and retained-calibration tooling source-complete; runtime calibration and canonical evidence pending**.

## Why this belongs in the Profiles improvement trail

Profiles privacy is owner-port policy, but Social Graph relationship events feed downstream Index behavior used by profile discovery. Raw poison handling therefore needs an explicit reliability boundary without turning broker state into profile authorization.

The existing external-Iggy cases show immediate suppression, disabled deduplication, capacity eviction, and expiry. Those short sequences do not prove that production `max_entries` and `expiry` cover the complete broker-success-to-recovery interval.

## Delivered source policy

`rustok-iggy` publishes:

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

## Delivered retained calibration boundary

The retained slice adds:

```text
crates/rustok-iggy/contracts/evidence/
  dedup-recovery-window-calibration-execution-contract.json
crates/rustok-iggy/tests/
  dedup_recovery_window_calibration.rs
scripts/evidence/
  capture-iggy-dedup-recovery-window-calibration.mjs
scripts/verify/
  verify-iggy-dedup-recovery-window-retained.mjs
```

The runner requires a versioned reviewed recovery-bounds JSON and a reviewed external Iggy configuration file outside the repository. It retains only:

- the bounded recovery inputs and checked required expiry;
- the reviewed per-partition capacity basis and required entry count;
- the canonical enabled/max-entries/expiry configuration projection;
- canonical projection digests;
- a bounded server artifact label;
- one exact sufficient Rust assessment;
- commit, toolchain, timestamps, current source hashes, and test-output digest/size.

It does not retain input paths, full files, full-file hashes, endpoints, credentials, payloads, UUIDs, partitions, offsets, acknowledgement tokens, or raw logs.

The exact Rust case skips when no calibration environment is present so ordinary test runs remain opt-in. The retained runner rejects that skip, requires `running 1 test`, requires the named case to pass, and refuses to write a packet unless both reviewed expiry and capacity cover the supplied model.

Packet publication is no-clobber and commit-bound. The canonical packet is intentionally absent until a maintainer performs the calibration.

## Profiles boundary

Profiles never authorizes visibility, `followers_only`, follow controls, search inclusion, or presentation from:

- deduplication enabled/disabled state;
- `max_entries` or `expiry`;
- recovery-window assessment status;
- reviewed bounds or configuration digests;
- receipt state, broker counts, offsets, lag, or evidence packets.

Privacy remains evaluated before localized and Media-backed presentation. Restricted or unavailable public rows remain absent.

A `sufficient` assessment means only that one reviewed configuration covers the supplied bounded model. It does not prove active server configuration, the capacity basis itself, a database/broker transaction, exactly-once delivery, failover, or multi-replica recovery.

## Remaining evidence

1. review and supply production lease, restart, reconnect, and operator-response bounds;
2. review the maximum distinct deterministic IDs per physical partition during that interval;
3. run the source and retained verifiers plus the exact calibration capture;
4. inspect and commit the generated no-clobber packet;
5. repeat whenever a bound source, configuration, or bounds input changes;
6. separately prove bundled mode, TLS/auth/failover, capacity pressure, and multi-replica claim ownership.

Tests, Cargo commands, source verifiers, retained capture, broker connections, and multi-replica scenarios were not run per maintainer instruction.
