# Bounded external-Iggy DLQ duplicate scan

Status: **global and fair-window source harnesses complete; runtime execution and retained evidence pending**.

## Purpose

`IggyDlqDuplicateScanner` adapts the transport-neutral physical duplicate
classifier to an already connected external `IggyClient`.

It supports two explicit bounded questions:

```text
global request:
  within these ordered DLQ partitions, starting at one explicit offset,
  what is the count-only summary for at most N messages total?

fair snapshot window:
  within every selected DLQ partition, starting at one explicit offset,
  what is the count-only summary for at most N messages per partition?
```

It does not discover the broker, own credentials, create topology, persist a
cursor, or perform reconciliation.

## Public API

```text
IggyDlqDuplicateScanRequest
IggyDlqDuplicateScanWindowPolicy
IggyDlqDuplicateScanner
IggyDlqDuplicateScanError
```

The result remains the identifier-free `DlqDuplicateSummary`.

Connection and authentication lifecycle remain caller-owned. The scanner borrows
an already connected `IggyClient` and never calls shutdown.

## Polling boundary

```text
consumer kind: standalone Consumer
consumer name: rustok-dlq-duplicate-readonly-v1
topic: dlq
partition: explicit positive ID
strategy: PollingStrategy::offset(explicit_offset)
auto_commit: false
```

The scanner does not use a consumer group or stored-offset `next` polling. It
also avoids topic discovery. Operators supply the reviewed partition allowlist.

## Compatibility global request

`IggyDlqDuplicateScanRequest` requires:

- 1 to 128 unique positive partition IDs;
- one explicit start offset;
- `max_messages` from 1 through 10,000;
- `batch_size` from 1 through 1,000;
- `batch_size <= max_messages`.

The message budget is shared across the ordered allowlist. A busy earlier
partition can consume the cap before later partitions are polled.

## Fair snapshot window

`IggyDlqDuplicateScanWindowPolicy` requires:

- 1 to 128 unique positive partition IDs;
- one explicit start offset;
- one positive `per_partition_messages`;
- `batch_size` from 1 through 1,000;
- `batch_size <= per_partition_messages`;
- checked `partition_count * per_partition_messages <= 10,000`.

A successful fair scan attempts every configured partition under the same cap
and combines all observations before classification.

The scanner can classify repeated IDs found in any combined observation set.
The production deterministic DLQ publisher, however, routes by the broker UUID:

```text
partition = (broker_message_id_as_u128 mod partition_count) + 1
```

Production copies with the same deterministic ID are therefore colocated in one
partition. Runtime evidence must distinguish the scanner's aggregate capability
from the production-reachable partition invariant; it must not claim that
`IggyTransport::move_to_dlq` split one ID across partitions.

The fair policy is one fixed snapshot. It does not add a moving cursor, stored
progress, cross-cycle identity/digest accumulation, current-tail coverage, or
complete-history proof.

## Response validation

Each physical poll response must satisfy:

- returned partition equals the request;
- reported count equals the returned messages;
- count does not exceed the requested batch;
- offsets are at or after the requested offset;
- offsets in one batch are strictly increasing;
- `last_offset + 1` does not overflow;
- every physical header ID is a non-nil UUID accepted by the classifier.

Any mismatch fails closed. Public errors do not copy raw client errors or broker
coordinates.

## Server integration

The mode-aware event-delivery observer supports:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=global_budget
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=fair_window
```

`global_budget` remains the compatibility default. `fair_window` is explicit
opt-in and requires:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_PER_PARTITION_MESSAGES
```

The integration does not change `outbox`, creates no second transport,
and cannot become a Profiles authorization input.

## Privacy boundary

The scanner temporarily passes physical header UUIDs and exact bytes to the
in-memory classifier. Exact bytes are immediately reduced to a domain-separated
SHA-256 value.

The returned summary exposes no broker address, stream/topic/partition/offset,
message UUID, payload/digest, credentials, or raw Iggy error.

Stable scanner codes:

```text
iggy.dlq_duplicate.scan_invalid
iggy.dlq_duplicate.scan_failed
iggy.dlq_duplicate.scan_response_invalid
iggy.dlq_duplicate.scan_offset_overflow
```

## Mutation boundary

The scanner contains no automatic offset commit, offset storage,
acknowledgement, topology deletion/purge, publication, replay/retry, receipt
mutation, or caller-client shutdown.

## Compatibility-global source harness

The existing `dlq_duplicate_external_scan` target publishes four messages through
production `IggyTransport::move_to_dlq` on one partition:

```text
A, A: same deterministic ID and same bytes
B1, B2: same deterministic ID and different bytes
```

The same `[partition 1, offset 0, max 4, batch 4]` request runs twice. Both
identifier-free summaries contain two duplicate groups, one conflicting group,
and no stored standalone-consumer offset.

Runtime execution remains pending.

## Fair-window multi-partition source harness

The new `dlq_duplicate_fair_window_external_scan` target uses two partitions and
five production-published messages:

```text
partition 1: A/A ordinary duplicate, then one unique overflow message
partition 2: B1/B2 conflicting-payload duplicate
```

Fair policy:

```text
partitions = [1, 2]
per_partition_messages = 2
batch_size = 2
```

It must observe both duplicate groups and the conflict. The compatibility global
request with `max_messages = 4` must instead consume three partition-1 messages
and only one partition-2 message, producing a different summary.

Both fair scans reuse offset zero. Stored offsets must remain absent for both
partitions before publication and after every scan.

The harness preserves production same-ID colocation and contains no direct SDK
producer.

Detailed evidence contract:

```text
crates/rustok-iggy/contracts/evidence/
  dlq-duplicate-fair-window-external-scan-runtime-source.json
```

## Source verification

```bash
node scripts/verify/verify-iggy-dlq-duplicate-external-scan.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan-runtime.mjs
node scripts/verify/verify-iggy-dlq-duplicate-fair-window-external-scan-runtime.mjs
node scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs
```

No test, Cargo command, formatter, verifier, broker connection, or runtime scan
was executed while defining these slices.

## Remaining work

1. execute the compatibility-global case on a reviewed dedup-disabled broker;
2. execute the two-partition fair-window case;
3. add and execute clean-commit retained capture for the fair-window case;
4. retain privacy-safe packets without addresses, identifiers, payloads, offsets,
   credentials, or raw logs;
5. design moving windows with bounded cross-cycle duplicate state, or keep fixed
   snapshots;
6. keep destructive reconciliation separately authorized;
7. preserve identifier-free correlation with poison receipt health.
