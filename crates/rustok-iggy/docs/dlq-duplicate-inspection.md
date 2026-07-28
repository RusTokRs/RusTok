# Count-only physical DLQ duplicate inspection

Status: **transport-neutral source complete; bounded external-Iggy scan adapter pending**.

## Purpose

The neutral poison receipt store answers whether a source delivery is reserved, publishing, published, or acknowledged. It does not answer how many physical copies currently exist in the Iggy `dlq` topic.

Physical copies can exceed one when:

- server-side message-ID deduplication is disabled;
- the deterministic ID expired from the dedup window;
- capacity pressure evicted the ID;
- a failover or unsupported broker path did not preserve the expected dedup state.

`dlq_duplicate_inspection.rs` provides a transport-neutral, read-only reduction for a bounded set of physical observations.

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

This must not be silently collapsed into an ordinary duplicate. It indicates header corruption, an invalid producer, an unsupported reuse of the deterministic ID, or an extraordinary hash/identity failure.

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

The classifier does not implement serialization. A future operator endpoint must preserve the same count-only projection unless a separately authorized forensic workflow is explicitly designed.

## Mutation boundary

The classifier cannot:

- acknowledge a DLQ cursor;
- delete a physical message;
- replay or retry a message;
- repair broker state;
- claim or release a poison receipt;
- mark a receipt published or acknowledged;
- choose alert thresholds or operator policy.

This separation is deliberate. Observation must not accidentally become destructive reconciliation.

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

## Safe operational sequence

A future bounded external-Iggy adapter should:

1. open a read-only scan cursor or metadata reader over an explicitly selected DLQ scope;
2. enforce a maximum message count and bounded time window;
3. extract only the physical header UUID and exact bytes into in-memory observations;
4. call `summarize_dlq_duplicates`;
5. publish only the count-only summary;
6. close the observer without acknowledging, deleting, replaying, or moving messages.

Any destructive action must be a separate, explicitly authorized workflow with its own preview, selection, audit, and retained evidence.

## Source tests

The focused unit cases define:

- same ID and exact bytes counted as ordinary physical duplicates;
- same ID and different bytes escalated as an identity conflict;
- empty scan and empty payload behavior;
- nil ID rejection and stable error code.

Suggested maintainer commands:

```bash
node scripts/verify/verify-iggy-dlq-duplicate-inspection.mjs
cargo test -p rustok-iggy dlq_duplicate_inspection -- --nocapture
```

No tests, Cargo commands, formatters, verifiers, or external-Iggy scans were run while defining this source slice.

## Remaining work

1. add a bounded read-only external-Iggy scan adapter;
2. prove physical header extraction and count-only projection against a disposable broker;
3. retain runtime evidence without identifiers, payloads, addresses, credentials, or raw logs;
4. define alert thresholds outside the inspector;
5. design any acknowledgement/delete/replay workflow as a separate authorized operation;
6. correlate aggregate receipt and duplicate health without exporting message identities.
