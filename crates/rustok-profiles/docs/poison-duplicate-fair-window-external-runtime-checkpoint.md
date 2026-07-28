# Profiles checkpoint: external fair-window DLQ duplicate runtime source

Status: **two-partition source harness complete; execution and retained evidence pending**.

## What was added

`rustok-iggy` now defines one opt-in production-path scenario for the explicit
`fair_window` duplicate scanner:

```text
crates/rustok-iggy/tests/dlq_duplicate_fair_window_external_scan.rs
crates/rustok-iggy/contracts/evidence/
  dlq-duplicate-fair-window-external-scan-runtime-source.json
scripts/verify/
  verify-iggy-dlq-duplicate-fair-window-external-scan-runtime.mjs
```

The scenario compares the fair policy with the compatibility global request over
the same five physical DLQ messages.

## Production-reachable fixture

The test uses only `IggyTransport::move_to_dlq`. It does not use a direct SDK
producer.

Production DLQ publication chooses:

```text
partition = (broker_message_id_as_u128 mod partition_count) + 1
```

Copies with the same deterministic broker ID are therefore colocated. The
runtime source does not claim that one ID is physically split across partitions.

Fixture shape:

```text
partition 1: ordinary duplicate A/A, then one unique overflow message
partition 2: conflicting-payload duplicate B1/B2
```

## Fair versus global result

The fair policy reads two messages from each partition and reports two duplicate
groups, including one conflicting-payload group.

The ordered global request reads three messages from partition 1 and one from
partition 2. Its identifier-free summary therefore has only one duplicate group
and no observed identity conflict.

This difference locks the equal per-partition budget without inventing a moving
cursor or a cross-partition same-ID fixture.

## Read-only boundary

Both scans use explicit offset zero and `auto_commit=false`. Stored consumer
offsets must remain absent for both partitions before publication and after every
scan.

The source harness cannot acknowledge, store/delete offsets, publish through the
observer SDK client, join a consumer group, delete/purge topology, mutate poison
receipts, or alter broker configuration.

## Profiles boundary

No profile visibility, relationship, follower, block, mute, ownership, audience,
storefront, author-card, or localized presentation decision may depend on:

- global or fair scan selection;
- partition counts or message caps;
- duplicate/conflict summaries;
- stored-offset observations;
- broker deduplication configuration;
- harness execution or retained evidence.

Profiles remains a consumer of authoritative owner ports only.

## Remaining work

1. execute the exact case on a reviewed dedup-disabled disposable broker;
2. add a clean-commit retained runner and privacy-safe packet;
3. execute and retain the compatibility-global case;
4. decide whether fixed snapshots are sufficient or design bounded moving-window
   state that preserves identities across cycles;
5. keep destructive reconciliation separately authorized.

No tests, Cargo commands, source verifiers, broker scans, or retained capture were
run by the implementation agent.
