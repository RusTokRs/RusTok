# Profiles checkpoint: physical DLQ duplicate operations

Status: **count-only classifier source-complete; external observer pending**.

## Why this matters for Profiles

Profiles authorization remains independent of broker and receipt state. However, downstream processing of relationship facts must not hide the difference between:

- one durable neutral result with one physical DLQ copy;
- one durable neutral result with repeated identical physical copies;
- one deterministic DLQ ID associated with conflicting exact bytes.

The new Iggy-owned classifier makes that difference visible without exposing message identities or payloads.

## New owner boundary

Source:

```text
crates/rustok-iggy/src/dlq_duplicate_inspection.rs
```

Machine contract:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-inspection-source.json
```

Verifier:

```text
scripts/verify/verify-iggy-dlq-duplicate-inspection.mjs
```

Public API:

```text
DlqDuplicateObservation
DlqDuplicateSummary
DlqDuplicateInspectionError
summarize_dlq_duplicates
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

## Mutation boundary

The classifier cannot acknowledge, delete, replay, retry, repair, claim, release, or mark anything. It cannot choose alert thresholds or operator policy.

A future external-Iggy scan adapter must remain read-only and bounded. Any destructive reconciliation must be a separate explicitly authorized workflow with preview and audit evidence.

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
- receipt recovery counts;
- deduplication configuration;
- retained evidence metadata.

Profiles presentation continues to consume authorized owner-port results. This classifier only improves operational observability of downstream neutralization.

## Remaining work

1. add a bounded read-only external-Iggy DLQ scan adapter;
2. prove physical header and byte ingestion against a disposable broker;
3. retain only count-level runtime evidence;
4. define alert thresholds outside the classifier;
5. define any acknowledgement/delete/replay workflow separately with explicit authorization;
6. keep aggregate receipt and duplicate observations identifier-free.

No runtime adapter or retained execution packet exists for this checkpoint. Tests and verifiers were not run by the implementation agent.
