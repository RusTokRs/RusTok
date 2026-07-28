# Profiles checkpoint: physical DLQ duplicate alert policy

Status: **count-only policy and latest-value runtime composition source-complete; server integration pending**.

## What changed

`rustok-iggy` owns a transport-neutral policy that evaluates the existing count-only physical DLQ duplicate summary and an in-memory latest-value runtime composition for the resulting identifier-free evaluation.

Policy source:

```text
crates/rustok-iggy/src/dlq_duplicate_alert_policy.rs
```

Runtime source:

```text
crates/rustok-iggy/src/dlq_duplicate_alert_runtime.rs
```

Machine contracts:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-policy-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-runtime-source.json
```

Verifiers:

```text
scripts/verify/verify-iggy-dlq-duplicate-alert-policy.mjs
scripts/verify/verify-iggy-dlq-duplicate-alert-runtime.mjs
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

## Latest-value runtime composition

The runtime sequence is:

```text
already observed DlqDuplicateSummary
  -> prevalidated DlqDuplicateAlertPolicy
  -> single-writer runtime publisher
  -> identifier-free latest snapshot
  -> read-only subscribers
```

The initial state is unavailable with generation `0` and no evaluation. Successful observation publishes an available evaluation. Observation failure or shutdown publishes unavailable and clears the old evaluation so stale severity does not remain current.

The channel retains only the latest state. It is not an audit log and does not promise that every subscriber sees every intermediate generation.

## Count-only runtime projection

The runtime snapshot exposes only:

```text
generation
available
evaluation
```

The optional evaluation exposes only:

```text
level
physical_duplicates
identity_conflict
duplicate_messages_threshold_reached
duplicate_groups_threshold_reached
max_copies_threshold_reached
```

It does not expose counts from the source summary, raw threshold values, broker coordinates, UUIDs, payloads, payload digests, receipt identities, credentials, timestamps, or raw Iggy errors.

No serialization or persistence is added.

## Profiles authorization boundary

No profile visibility, ownership, follower access, block, mute, relationship, audience, storefront presentation, or author-card decision may depend on:

- `DlqDuplicateAlertLevel`;
- runtime availability or generation;
- any threshold-reached boolean;
- physical duplicate presence;
- identity-conflict presence;
- scanner availability or failure;
- threshold configuration;
- alert delivery state;
- retained evidence metadata.

Profiles continues to resolve privacy through authoritative owner ports and presents only approved owner results.

## Operational separation

The policy/runtime cannot:

- poll Iggy;
- inspect PostgreSQL receipts;
- start a server worker;
- register telemetry or health policy;
- send or page;
- choose a destination;
- persist thresholds or snapshots;
- schedule scans;
- affect readiness;
- acknowledge, delete, purge, replay, retry, or publish;
- claim, release, publish, or acknowledge a poison receipt;
- alter broker configuration or profile state.

Server observation, telemetry projection, notification delivery, cooldown/suppression, and destructive reconciliation remain separate owner boundaries.

## Remaining work

1. integrate an explicitly owned server observer;
2. define reviewed telemetry and health projection;
3. define alert routing, cooldown, and suppression outside Profiles and outside the policy/runtime;
4. retain identifier-free runtime integration evidence;
5. keep destructive reconciliation in a separate authorized workflow;
6. compare receipt and duplicate health only as aggregate operational trends.

No tests, Cargo commands, formatters, verifiers, server observers, external-Iggy scans, alert delivery, or retained capture were run by the implementation agent.
