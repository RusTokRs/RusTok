# Profiles checkpoint: physical DLQ duplicate alert policy

Status: **count-only policy source-complete; runtime integration pending**.

## What changed

`rustok-iggy` now owns a transport-neutral policy that evaluates the existing count-only physical DLQ duplicate summary.

Source:

```text
crates/rustok-iggy/src/dlq_duplicate_alert_policy.rs
```

Machine contract:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-policy-source.json
```

Verifier:

```text
scripts/verify/verify-iggy-dlq-duplicate-alert-policy.mjs
```

Public API:

```text
DlqDuplicateAlertPolicy
DlqDuplicateAlertLevel
DlqDuplicateAlertEvaluation
DlqDuplicateAlertPolicyError
```

No Profiles API, database table, GraphQL field, storefront behavior, privacy port, or authorization input changed.

## Explicit operator thresholds

The policy requires explicit warning and critical thresholds for:

```text
duplicate_messages
duplicate_groups
max_copies_per_message_id
```

There is no library-owned production default. Invalid zero, inverted, or impossible max-copies thresholds fail closed.

This preserves ownership: Profiles does not select operational duplicate tolerance, and `rustok-iggy` does not invent a tenant or product policy.

## Level semantics

```text
Clear    no physical duplicate
Notice   duplicates exist below warning thresholds
Warning  one or more warning thresholds reached
Critical one or more critical thresholds reached
         OR any identity conflict exists
```

Identity conflict means one deterministic physical message UUID was observed with different exact bytes. It is always `Critical` and `requires_manual_review()` is true.

A numeric `Critical` result without an identity conflict does not by itself authorize manual payload inspection or destructive reconciliation.

## Count-only projection

The evaluation exposes only:

```text
level
physical_duplicates
identity_conflict
duplicate_messages_threshold_reached
duplicate_groups_threshold_reached
max_copies_threshold_reached
```

It does not expose counts from the source summary, raw threshold values, broker coordinates, UUIDs, payloads, payload digests, receipt identities, credentials, timestamps, or raw Iggy errors.

## Profiles authorization boundary

No profile visibility, ownership, follower access, block, mute, relationship, audience, storefront presentation, or author-card decision may depend on:

- `DlqDuplicateAlertLevel`;
- any threshold-reached boolean;
- physical duplicate presence;
- identity-conflict presence;
- scanner availability or failure;
- threshold configuration;
- alert delivery state;
- retained evidence metadata.

Profiles continues to resolve privacy through authoritative owner ports and presents only approved owner results.

## Operational separation

The intended sequence remains:

```text
external bounded scan
  -> DlqDuplicateSummary
  -> DlqDuplicateAlertPolicy::evaluate
  -> identifier-free evaluation
  -> separately owned notification/suppression runtime
```

The policy itself cannot:

- poll Iggy;
- inspect PostgreSQL receipts;
- send or page;
- choose a destination;
- persist thresholds;
- schedule scans;
- acknowledge, delete, purge, replay, retry, or publish;
- claim, release, publish, or acknowledge a poison receipt;
- alter broker configuration.

## Remaining work

1. integrate the policy into an explicitly owned runtime observer;
2. define alert routing, cooldown, and suppression outside Profiles and outside the policy;
3. retain identifier-free runtime policy evidence;
4. keep destructive reconciliation in a separate authorized workflow;
5. compare receipt and duplicate health only as aggregate operational trends.

No tests, Cargo commands, formatters, verifiers, external-Iggy scans, alert delivery, or retained capture were run by the implementation agent.
