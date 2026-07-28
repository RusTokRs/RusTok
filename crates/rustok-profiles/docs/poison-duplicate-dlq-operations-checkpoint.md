# Profiles checkpoint: physical DLQ duplicate operations

Status: **count-only classifier and bounded external observer source-complete; runtime evidence pending**.

## Why this matters for Profiles

Profiles authorization remains independent of broker and receipt state. However, downstream processing of relationship facts must not hide the difference between:

- one durable neutral result with one physical DLQ copy;
- one durable neutral result with repeated identical physical copies;
- one deterministic DLQ ID associated with conflicting exact bytes.

The Iggy-owned classifier makes that difference visible without exposing message identities or payloads. The bounded external scanner now supplies physical observations through explicit-offset polling without storing progress.

## Owner boundaries

Classifier source:

```text
crates/rustok-iggy/src/dlq_duplicate_inspection.rs
```

External scanner source:

```text
crates/rustok-iggy/src/dlq_duplicate_external_scan.rs
```

Machine contracts:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-inspection-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-source.json
```

Verifiers:

```text
scripts/verify/verify-iggy-dlq-duplicate-inspection.mjs
scripts/verify/verify-iggy-dlq-duplicate-external-scan.mjs
```

Public API:

```text
DlqDuplicateObservation
DlqDuplicateSummary
DlqDuplicateInspectionError
summarize_dlq_duplicates
IggyDlqDuplicateScanRequest
IggyDlqDuplicateScanner
IggyDlqDuplicateScanError
```

## Count-only projection

The summary exposes only:

```text
total_messages
unique_message_ids
duplicate_messages
duplicate_groups
conflicting_payload_groups
max_copies_per_message_id
```

It excludes broker endpoints, stream coordinates, UUIDs, payloads, payload digests, receipt identities, error codes, publisher identities, timestamps, and credentials.

## Duplicate classification

An ordinary physical duplicate requires:

```text
same deterministic Iggy header UUID
same exact bytes
```

The bytes are compared through an in-memory domain-separated SHA-256 digest that is never exposed by the public observation or summary.

A repeated UUID with different exact bytes increments `conflicting_payload_groups` and requires manual review. It is not silently collapsed into a normal duplicate.

## External scan boundary

The scanner borrows an already connected `IggyClient` and uses:

```text
topic = dlq
standalone consumer
explicit partition
explicit offset
auto_commit = false
```

It accepts no more than 128 unique positive partitions, 10,000 physical messages globally, and batches of 1,000. It validates returned partition/count and monotonic offsets before returning only `DlqDuplicateSummary`.

The scanner does not use a consumer group, stored-offset `next` polling, offset storage, acknowledgement, topology discovery, publication, delete/purge, replay/retry, receipt mutation, or client shutdown.

## Relationship to receipt health

The PostgreSQL `ConsumerPoisonReceiptInspector` remains an independent count-only summary of receipt progress. Neither side exports identifiers, so the two views cannot be joined message by message.

Aggregate operational interpretation may compare trends:

- expired publishing claims plus duplicate growth may indicate recovery outside the effective dedup window;
- duplicate growth without recovery work may reflect historic or downstream duplication;
- any conflicting-payload group requires forensic escalation.

These interpretations never become Profiles authorization inputs.

## Profiles authorization boundary

No profile visibility, relationship, block, mute, follow, friendship, audience, or presentation decision may depend on:

- physical DLQ copy count;
- duplicate group count;
- conflicting-payload group count;
- scan partition or offset selection;
- receipt recovery counts;
- deduplication configuration;
- retained evidence metadata.

Profiles presentation continues to consume authorized owner-port results. This classifier and scanner only improve operational observability of downstream neutralization.

## Remaining work

1. prove physical header and exact-byte ingestion against a disposable external broker;
2. prove `auto_commit=false` explicit-offset polling leaves no stored progress;
3. retain only count-level runtime evidence;
4. define alert thresholds outside the classifier and Profiles;
5. define acknowledgement/delete/replay separately with explicit authorization;
6. keep aggregate receipt and duplicate observations identifier-free.

No retained execution packet exists for this checkpoint. Tests, Cargo commands, formatters, verifiers, and external-Iggy scans were not run by the implementation agent.
