# Profiles checkpoint: physical DLQ duplicate operations

Status: **count-only classifier, bounded external observer, runtime harness, retained tooling, and alert policy source-complete; runtime execution and integration pending**.

## Why this matters for Profiles

Profiles authorization remains independent of broker and receipt state. However, downstream processing of relationship facts must not hide the difference between:

- one durable neutral result with one physical DLQ copy;
- one durable neutral result with repeated identical physical copies;
- one deterministic DLQ ID associated with conflicting exact bytes.

The Iggy-owned classifier makes that difference visible without exposing message identities or payloads. The bounded external scanner supplies physical observations through explicit-offset polling without storing progress. The separate alert policy evaluates only the count-only summary and does not become a Profiles input.

## Owner boundaries

Classifier source:

```text
crates/rustok-iggy/src/dlq_duplicate_inspection.rs
```

External scanner source:

```text
crates/rustok-iggy/src/dlq_duplicate_external_scan.rs
```

Alert policy source:

```text
crates/rustok-iggy/src/dlq_duplicate_alert_policy.rs
```

Machine contracts:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-inspection-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-policy-source.json
```

Verifiers:

```text
scripts/verify/verify-iggy-dlq-duplicate-inspection.mjs
scripts/verify/verify-iggy-dlq-duplicate-external-scan.mjs
scripts/verify/verify-iggy-dlq-duplicate-alert-policy.mjs
```

Public API additionally includes:

```text
DlqDuplicateAlertPolicy
DlqDuplicateAlertLevel
DlqDuplicateAlertEvaluation
DlqDuplicateAlertPolicyError
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

The policy evaluation exposes only:

```text
level
physical_duplicates
identity_conflict
duplicate_messages_threshold_reached
duplicate_groups_threshold_reached
max_copies_threshold_reached
```

Both projections exclude broker endpoints, stream coordinates, UUIDs, payloads, payload digests, receipt identities, producer identities, credentials, timestamps, and raw Iggy errors. The evaluation also excludes source counts and raw threshold values.

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

## Alert policy boundary

The caller must supply warning and critical thresholds for duplicate messages, duplicate groups, and max copies per message ID. The library provides no production defaults.

Level precedence is:

```text
identity conflict -> Critical
critical numeric threshold -> Critical
warning numeric threshold -> Warning
physical duplicate below warning -> Notice
no duplicate -> Clear
```

The policy does not choose notification routing, paging, cooldown, suppression, scan cadence, threshold persistence, or any destructive action.

## Runtime and retained status

The external-Iggy runtime harness and retained execution tooling are source-complete. The harness creates controlled ordinary-duplicate and identity-conflict fixtures through production publication, scans the same explicit offset twice, and requires no stored consumer offset before or after either scan.

The canonical retained execution packet remains absent until a maintainer performs the reviewed external-Iggy run.

## Relationship to receipt health

The PostgreSQL `ConsumerPoisonReceiptInspector` remains an independent count-only summary of receipt progress. Neither side exports identifiers, so the two views cannot be joined message by message.

Aggregate operational interpretation may compare trends:

- expired publishing claims plus duplicate growth may indicate recovery outside the effective dedup window;
- duplicate growth without recovery work may reflect historic or downstream duplication;
- any identity conflict requires forensic escalation.

These interpretations never become Profiles authorization inputs.

## Profiles authorization boundary

No profile visibility, relationship, block, mute, follow, friendship, audience, or presentation decision may depend on:

- physical DLQ copy or duplicate-group counts;
- identity-conflict presence;
- alert level or threshold flags;
- scan partition or offset selection;
- scanner or alert-delivery state;
- receipt recovery counts;
- deduplication or alert-threshold configuration;
- retained evidence metadata.

Profiles presentation continues to consume authorized owner-port results. These components only improve operational observability of downstream neutralization.

## Remaining work

1. execute and retain the reviewed external-Iggy duplicate scan packet;
2. integrate the pure alert policy into an explicitly owned runtime observer;
3. define alert routing, cooldown, and suppression outside Profiles and the policy;
4. define acknowledgement/delete/replay separately with explicit authorization;
5. keep aggregate receipt and duplicate observations identifier-free.

Tests, Cargo commands, formatters, verifiers, external-Iggy scans, alert dispatch, and retained capture were not run by the implementation agent.
