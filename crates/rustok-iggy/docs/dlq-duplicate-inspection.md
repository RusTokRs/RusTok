# Count-only physical DLQ duplicate inspection

Status: **classifier, bounded external-Iggy adapter, runtime harness, retained tooling, alert policy, and latest-value alert runtime source-complete; runtime execution and server integration pending**.

## Purpose

The neutral poison receipt store answers whether a source delivery is reserved, publishing, published, or acknowledged. It does not answer how many physical copies currently exist in the Iggy `dlq` topic.

Physical copies can exceed one when:

- server-side message-ID deduplication is disabled;
- the deterministic ID expired from the dedup window;
- capacity pressure evicted the ID;
- a failover or unsupported broker path did not preserve the expected dedup state.

`dlq_duplicate_inspection.rs` provides a transport-neutral, read-only reduction for bounded physical observations. `dlq_duplicate_external_scan.rs` provides the bounded external-Iggy adapter. `dlq_duplicate_alert_policy.rs` evaluates the count-only summary against explicit thresholds. `dlq_duplicate_alert_runtime.rs` publishes the latest identifier-free evaluation state for read-only consumers.

## Input boundary

Each observation accepts:

```text
non-nil deterministic Iggy message header UUID
exact physical payload bytes
```

The exact bytes are immediately reduced to a domain-separated SHA-256 value in memory. The observation exposes neither the bytes nor their digest. Empty payloads are valid because empty raw poison bytes are already valid at the receipt boundary.

A nil UUID fails closed with:

```text
iggy.dlq_duplicate.identity_invalid
```

## Count-only result

`DlqDuplicateSummary` exposes only:

```text
total_messages
unique_message_ids
duplicate_messages
duplicate_groups
conflicting_payload_groups
max_copies_per_message_id
```

Where:

```text
duplicate_messages = total_messages - unique_message_ids
```

A duplicate group has more than one physical observation for the same deterministic message ID.

## Ordinary duplicate versus identity conflict

### Ordinary physical duplicate

```text
same non-nil message header UUID
same exact payload digest
```

This is the expected shape of a publish/mark ambiguity retry when the broker accepts the same deterministic ID more than once.

### Identity conflict

```text
same non-nil message header UUID
different exact payload digests
```

This must not be silently collapsed into an ordinary duplicate. It indicates header corruption, an invalid producer, unsupported reuse of the deterministic ID, or an extraordinary hash/identity failure.

The summary increments `conflicting_payload_groups` and `requires_manual_review()` returns `true`.

## Privacy boundary

The summary does not expose:

- broker address;
- stream, topic, partition, or offset;
- deterministic message UUID;
- payload or payload digest;
- receipt identity or state;
- error classification;
- publisher identity;
- timestamps;
- credentials.

The classifier does not implement serialization. Any operator endpoint must preserve the same count-only projection unless a separately authorized forensic workflow is explicitly designed.

## Mutation boundary

The classifier and external scanner cannot:

- acknowledge a DLQ cursor;
- store or auto-commit a consumer offset;
- delete or purge physical messages;
- replay or retry a message;
- publish a message;
- repair broker state;
- claim or release a poison receipt;
- mark a receipt published or acknowledged;
- choose alert thresholds or operator policy.

The separate alert policy accepts explicit caller thresholds but cannot scan, send notifications, choose routing/cooldown, persist policy, or mutate state. The alert runtime only replaces one in-memory latest-value snapshot and cannot start a worker, register telemetry/health, deliver notifications, or mutate broker/receipt/Profile state.

This separation is deliberate. Observation, evaluation, runtime publication, delivery, and reconciliation must remain distinct.

## Relationship to deterministic raw poison identity

`ConsumedContractDecodeFailure::delivery_id` derives one UUID from immutable source stream/topic/partition/offset and exact raw bytes. Failure kind, retry count, process identity, time, and random values are excluded.

`to_dlq_entry` attaches that UUID as the Iggy broker message ID. `IggyTransport::move_to_dlq` uses the deterministic publisher path when this ID is present.

The duplicate classifier therefore groups physical Iggy messages by the same identity used for server-side deduplication.

## Relationship to receipt health

`ConsumerPoisonReceiptInspector` remains an independent count-only view of PostgreSQL receipt progress:

```text
reserved
publishing
expired_publishing
published
acknowledged
```

Neither summary contains identifiers, so they cannot be joined message by message. Operators may compare aggregate trends, but those interpretations are not storage or Profiles authorization decisions.

## Bounded external-Iggy adapter

`IggyDlqDuplicateScanner` borrows an already connected `IggyClient` and polls only:

```text
topic = dlq
standalone consumer
explicit positive partition
PollingStrategy::offset(explicit_offset)
auto_commit = false
```

One request permits at most 128 partitions, 10,000 messages globally, and batches of 1,000. It validates returned partition, count, monotonic offsets, and header identity before returning only `DlqDuplicateSummary`.

The adapter does not own credentials or connection lifecycle, query topology metadata, join a consumer group, persist progress, or call shutdown.

## Count-only alert policy

`DlqDuplicateAlertPolicy` requires explicit warning and critical thresholds for duplicate messages, duplicate groups, and maximum copies for one message ID. It defines no production defaults.

Evaluation order is:

```text
identity conflict -> Critical
critical numeric threshold -> Critical
warning numeric threshold -> Warning
physical duplicate below warning -> Notice
no duplicate -> Clear
```

The evaluation exposes only level and boolean reason flags. It does not expose source counts, raw threshold values, identifiers, or broker coordinates.

## Latest-value alert runtime

`DlqDuplicateAlertRuntimePublisher` accepts an already-observed `DlqDuplicateSummary` and a prevalidated policy.

```text
summary -> policy evaluate -> latest runtime snapshot -> read-only subscribers
```

The publisher is single-writer. The initial snapshot is unavailable at generation `0` with no evaluation. Every successful or unavailable transition increments generation through checked arithmetic.

`mark_unavailable()` clears the previous evaluation, preventing a stale `Warning` or `Critical` value from appearing current after observer failure or shutdown.

The snapshot exposes only:

```text
generation
available
evaluation
```

The runtime is a latest-value channel, not an event log. It does not promise delivery of every intermediate generation and adds no serialization or persistence.

## Safe operational sequence

1. select an external service, stream, explicit partition allowlist, offset, and message cap;
2. connect and authenticate an `IggyClient` outside the scanner;
3. call the bounded scanner with explicit-offset polling and `auto_commit=false`;
4. pass the count-only summary to an explicitly configured alert policy;
5. publish the evaluation through the single-writer runtime;
6. expose only read-only runtime subscribers to separately reviewed telemetry/health adapters;
7. close the client through the caller-owned lifecycle;
8. treat the result as a bounded window, not automatically as complete history.

Notification delivery, cooldown/suppression, and destructive actions require separate owner contracts.

## Runtime and retained evidence status

The opt-in external harness is source-complete. It uses production `move_to_dlq` to create ordinary duplicate and identity-conflict fixtures, scans the same explicit offset twice, and requires the scanner's standalone consumer offset to remain absent before and after both scans.

The clean-commit retained contract, runner, verifier, reviewed dedup-disabled configuration boundary, source hashes, exact-case execution, and privacy-safe packet projection are also source-complete.

The canonical execution JSON remains absent until a maintainer runs the reviewed external-Iggy scenario successfully. Server observer and telemetry/health integration for the alert runtime remain pending.

## Source tests and guards

Suggested maintainer commands:

```bash
node scripts/verify/verify-iggy-dlq-duplicate-inspection.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan-runtime.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan-retained.mjs
node scripts/verify/verify-iggy-dlq-duplicate-alert-policy.mjs
node scripts/verify/verify-iggy-dlq-duplicate-alert-runtime.mjs
cargo test -p rustok-iggy dlq_duplicate -- --nocapture
```

No tests, Cargo commands, formatters, verifiers, external-Iggy scans, server observers, telemetry registration, alert dispatch, or retained capture were run while defining these source slices.

## Remaining work

1. execute and retain the reviewed external-Iggy duplicate scan packet;
2. integrate an explicitly owned server alert observer;
3. define identifier-free telemetry and health projection;
4. define alert routing, cooldown, and suppression outside the classifier/policy/runtime;
5. design acknowledgement/delete/replay as a separate authorized operation;
6. correlate aggregate receipt and duplicate health without exporting message identities.
