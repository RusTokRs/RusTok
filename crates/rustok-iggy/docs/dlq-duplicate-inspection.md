# Count-only physical DLQ duplicate inspection

Status: **classifier, bounded external-Iggy adapter, runtime harness, retained tooling, and alert policy source-complete; runtime execution and integration pending**.

## Purpose

The neutral poison receipt store answers whether a source delivery is reserved, publishing, published, or acknowledged. It does not answer how many physical copies currently exist in the Iggy `dlq` topic.

Physical copies can exceed one when:

- server-side message-ID deduplication is disabled;
- the deterministic ID expired from the dedup window;
- capacity pressure evicted the ID;
- a failover or unsupported broker path did not preserve the expected dedup state.

`dlq_duplicate_inspection.rs` provides a transport-neutral, read-only reduction for a bounded set of physical observations. `dlq_duplicate_external_scan.rs` provides the bounded external-Iggy polling adapter. `dlq_duplicate_alert_policy.rs` separately evaluates the count-only summary against explicit operator thresholds.

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

The separate alert policy accepts explicit caller thresholds, but it cannot scan, send notifications, choose routing/cooldown, persist policy, or perform any mutation.

This separation is deliberate. Observation and evaluation must not accidentally become destructive reconciliation.

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

Neither summary contains identifiers, so they cannot be joined message-by-message. An operator may compare aggregate trends, for example:

- rising `expired_publishing` with rising physical duplicates suggests lease recovery outside the effective dedup window;
- zero receipt recovery work with growing duplicates suggests historic or downstream DLQ duplication;
- `conflicting_payload_groups > 0` always requires direct forensic escalation.

These are operator interpretations, not storage-layer decisions.

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

The adapter does not own credentials or connection lifecycle, query topology metadata, join a consumer group, persist progress, or call shutdown. See `dlq-duplicate-external-scan.md` for the complete contract.

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

The evaluation exposes only level and boolean reason flags. It does not expose source counts, raw threshold values, identifiers, or broker coordinates. See `dlq-duplicate-alert-policy.md`.

## Safe operational sequence

1. select an external service, stream, explicit partition allowlist, offset, and message cap;
2. connect and authenticate an `IggyClient` outside the scanner;
3. call the bounded scanner using explicit-offset polling with `auto_commit=false`;
4. pass the count-only summary to an explicitly configured alert policy when evaluation is required;
5. publish only the identifier-free summary/evaluation through a separately owned delivery layer;
6. close the client through the caller-owned lifecycle;
7. treat the result as a bounded window, not automatically as complete history.

Any destructive action must be a separate, explicitly authorized workflow with its own preview, selection, audit, and retained evidence.

## Runtime and retained evidence status

The opt-in external harness is source-complete. It uses production `move_to_dlq` to create ordinary duplicate and identity-conflict fixtures, scans the same explicit offset twice, and requires the scanner's standalone consumer offset to remain absent before and after both scans.

The clean-commit retained contract, runner, verifier, reviewed dedup-disabled configuration boundary, source hashes, exact-case execution, and privacy-safe packet projection are also source-complete.

The canonical execution JSON remains absent until a maintainer runs the reviewed external-Iggy scenario successfully.

## Source tests and guards

Suggested maintainer commands:

```bash
node scripts/verify/verify-iggy-dlq-duplicate-inspection.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan-runtime.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan-retained.mjs
node scripts/verify/verify-iggy-dlq-duplicate-alert-policy.mjs
cargo test -p rustok-iggy dlq_duplicate -- --nocapture
```

No tests, Cargo commands, formatters, verifiers, external-Iggy scans, alert dispatch, or retained capture were run while defining these source slices.

## Remaining work

1. execute and retain the reviewed external-Iggy duplicate scan packet;
2. integrate the pure alert policy into an explicitly owned runtime observer;
3. define alert routing, cooldown, and suppression outside the classifier and policy;
4. design acknowledgement/delete/replay as a separate authorized operation;
5. correlate aggregate receipt and duplicate health without exporting message identities.
