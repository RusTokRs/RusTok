# Profiles checkpoint: external fair-window DLQ duplicate retained capture

Status: **two-partition harness and retained-capture tooling source complete; execution and canonical evidence pending**.

## Delivered source

`rustok-iggy` now contains:

```text
crates/rustok-iggy/tests/
  dlq_duplicate_fair_window_external_scan.rs
crates/rustok-iggy/contracts/evidence/
  dlq-duplicate-fair-window-external-scan-runtime-source.json
  dlq-duplicate-fair-window-external-scan-execution-contract.json
scripts/evidence/
  capture-iggy-dlq-duplicate-fair-window-external-scan.mjs
scripts/verify/
  verify-iggy-dlq-duplicate-fair-window-external-scan-runtime.mjs
  verify-iggy-dlq-duplicate-fair-window-external-scan-retained.mjs
```

The canonical execution packet remains absent until the exact case is run on a
reviewed broker.

## Production-reachable scenario

The test publishes only through `IggyTransport::move_to_dlq`.

Production routing is:

```text
partition = (broker_message_id_as_u128 mod partition_count) + 1
```

Copies with the same deterministic broker ID are therefore colocated. The
scenario does not claim a same-ID cross-partition fixture.

Fixture shape:

```text
partition 1: ordinary duplicate A/A, then one unique overflow message
partition 2: conflicting-payload duplicate B1/B2
```

The fair policy reads two messages from each partition. The ordered global
request reads three messages from partition 1 and one from partition 2. Their
identifier-free summaries must differ, and the same fair policy must produce the
same summary twice from offset zero.

## Read-only proof

Stored standalone-consumer offsets must remain absent for both partitions:

```text
before publication
after first fair scan
after global scan
after second fair scan
```

The packet retains only `partitions_checked = 2` and zero stored-offset counts.
It does not retain partition IDs or offset values.

The harness cannot acknowledge, store/delete offsets, join a consumer group,
delete or purge topology, replay, retry, mutate poison receipts, or alter broker
configuration.

## Clean-commit capture

The execution contract requires:

- one exact Cargo case and `running 1 test`;
- rejection of skipped execution;
- a clean worktree before the run and after the test;
- one full unchanged Git commit;
- unchanged hashes for every bound source;
- a reviewed external Iggy artifact label;
- reviewed `message_deduplication.enabled = false`;
- bounded Cargo/Rust toolchain labels;
- fair and global summary assertions;
- four aggregate absent-offset checkpoints over two partitions;
- test-output SHA-256 and byte count.

The configuration path must point outside the repository. Only the canonical
deduplication section, disabled value, and its canonical digest are retained.

Packet publication is no-clobber: the runner exclusively creates a temporary
file and hard-links it to the canonical path. Existing evidence cannot be
silently replaced.

## Privacy boundary

The packet excludes broker endpoints, configuration paths/content, credentials,
connection strings, raw output, stream names, partition IDs, offsets, UUIDs,
payloads/digests, acknowledgement tokens, and raw Iggy errors.

It retains only bounded provenance, current source hashes, reviewed
configuration projection, toolchain/artifact labels, timestamps, identifier-free
summaries, aggregate absent-offset assertions, and output digest/size.

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

1. run the exact fair-window case on a reviewed dedup-disabled disposable broker;
2. inspect and commit the generated canonical packet;
3. execute and retain the compatibility-global packet separately;
4. decide whether fixed snapshots are sufficient or design bounded moving-window
   state that preserves identities across cycles;
5. keep destructive reconciliation separately authorized.

No tests, Cargo commands, source verifiers, broker scans, or retained capture
were run by the implementation agent.
