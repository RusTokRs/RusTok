# Profiles checkpoint: bounded external DLQ duplicate scan

Status: **global and fair-window source harnesses complete; execution and retained evidence pending**.

## Owner boundary

`rustok-iggy` owns the bounded read-only adapter and returns only
`DlqDuplicateSummary`.

```text
IggyDlqDuplicateScanRequest
IggyDlqDuplicateScanWindowPolicy
IggyDlqDuplicateScanner
IggyDlqDuplicateScanError
```

No Profiles API or authorization input was added.

## Read-only polling

```text
topic = dlq
consumer kind = standalone Consumer
polling strategy = explicit offset
partition = explicit positive ID
auto_commit = false
```

The scanner does not join a consumer group, use stored-offset `next` polling,
store an offset, acknowledge, discover topology, publish, delete, purge, replay,
retry, or shut down the caller-owned client.

## Global and fair policies

The compatibility global request has one cap shared by the ordered partition
allowlist. An early partition may consume it.

The fair policy gives every selected partition the same positive cap and checks:

```text
partition_count * per_partition_messages <= 10000
batch_size <= per_partition_messages
```

All observations are combined before classification.

## Production partition invariant

Production deterministic DLQ publication uses:

```text
partition = (broker_message_id_as_u128 mod partition_count) + 1
```

Physical copies with the same broker UUID are colocated. The scanner can combine
arbitrary partition observations, but production runtime evidence must not claim
that `IggyTransport::move_to_dlq` split one deterministic ID across partitions.

This invariant does not weaken the reason for fair budgets: one busy partition
must not prevent other partitions from being inspected.

## Compatibility-global source case

The existing one-partition case publishes ordinary and conflicting duplicates,
runs the same global request twice, and requires the scanner offset to remain
absent.

Execution and retained capture are pending.

## Multi-partition fair-window source case

The new source harness publishes through production `IggyTransport::move_to_dlq`:

```text
partition 1: A/A ordinary duplicate plus one unique overflow message
partition 2: B1/B2 conflicting-payload duplicate
```

The fair policy reads two messages per partition. It therefore observes both
duplicate groups and the partition-2 conflict.

The compatibility global request reads three messages from partition 1 and only
one from partition 2. Its summary must differ and must not observe the conflict.

The same fair policy runs twice from offset zero. Consumer offsets must remain
absent on partitions 1 and 2 before publication and after every scan.

Sources:

```text
crates/rustok-iggy/tests/dlq_duplicate_fair_window_external_scan.rs
crates/rustok-iggy/contracts/evidence/
  dlq-duplicate-fair-window-external-scan-runtime-source.json
scripts/verify/
  verify-iggy-dlq-duplicate-fair-window-external-scan-runtime.mjs
```

## Fixed-window limitation

Neither global nor fair mode owns moving cursors, persisted per-partition
progress, cross-cycle identity state, current-tail coverage, or complete-history
proof.

Repeated fixed scans are intentionally idempotent observations of the same
configured window.

## Profiles authorization

No profile visibility, relationship, follower, block, mute, ownership, audience,
storefront, author-card, or localized presentation result may depend on:

- scan mode, partitions, offsets, caps, or availability;
- duplicate/conflict counts;
- stored-offset observations;
- broker configuration;
- source harness or retained evidence status.

Profiles continues to consume authoritative owner-port results only.

## Privacy

Summaries and future packets exclude broker addresses, credentials, stream/topic
coordinates, UUIDs, payloads/digests, offsets, receipt identities, and raw client
errors.

## Remaining work

1. execute and retain the compatibility-global case;
2. execute the two-partition fair-window case;
3. add a clean-commit retained runner and packet for fair-window evidence;
4. choose fixed snapshots or a bounded moving-window design with cross-cycle
   identity state;
5. keep acknowledgement/delete/replay separately authorized.

No tests, Cargo commands, formatters, source verifiers, external-Iggy scans, or
retained capture were run by the implementation agent.
