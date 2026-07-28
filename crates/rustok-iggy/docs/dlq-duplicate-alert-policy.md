# Count-only physical DLQ duplicate alert policy

Status: **policy and latest-value runtime composition source-complete; server integration and retained evidence pending**.

## Purpose

`DlqDuplicateAlertPolicy` converts one identifier-free `DlqDuplicateSummary` into a deterministic count-only alert evaluation.

The policy does not scan Iggy, read PostgreSQL receipts, choose notification channels, send alerts, persist thresholds, or perform reconciliation. It only answers:

```text
Given these explicit operator thresholds and this count-only summary,
which alert level applies and which aggregate dimension reached it?
```

## Public API

```text
DlqDuplicateAlertPolicy
DlqDuplicateAlertLevel
DlqDuplicateAlertEvaluation
DlqDuplicateAlertPolicyError
```

The policy is transport-neutral and is not feature-gated on the Iggy SDK.

## Explicit thresholds

The caller must provide six values:

```text
warning_duplicate_messages
critical_duplicate_messages
warning_duplicate_groups
critical_duplicate_groups
warning_max_copies_per_message_id
critical_max_copies_per_message_id
```

No production default is provided. This prevents the library from silently deciding an operator's traffic, retention, or incident-response tolerance.

Validation fails closed unless:

- duplicate-message warning is at least `1`;
- duplicate-message critical is not below warning;
- duplicate-group warning is at least `1`;
- duplicate-group critical is not below warning;
- max-copies warning is at least `2`;
- max-copies critical is not below warning.

Invalid configuration returns:

```text
iggy.dlq_duplicate.alert_policy_invalid
```

Equal warning and critical thresholds are valid. Critical evaluation has precedence, so equality intentionally produces `Critical` when that boundary is reached.

## Alert levels

The ordered levels are:

```text
Clear
Notice
Warning
Critical
```

Stable codes:

```text
iggy.dlq_duplicate.alert.clear
iggy.dlq_duplicate.alert.notice
iggy.dlq_duplicate.alert.warning
iggy.dlq_duplicate.alert.critical
```

### Clear

No physical duplicate exists.

### Notice

Physical duplicates exist, but no warning threshold is reached.

`Notice` preserves visibility for a non-zero duplicate condition without allowing the library to decide that the condition must page or notify anyone.

### Warning

At least one warning threshold is reached and no critical condition is present.

### Critical

At least one critical numeric threshold is reached, or the summary contains an identity conflict.

Identity conflict always has precedence because one deterministic header UUID was observed with different exact payload bytes. It remains `Critical` even when every numeric threshold is much higher than the current counts.

## Evaluation projection

`DlqDuplicateAlertEvaluation` exposes only:

```text
level
physical_duplicates
identity_conflict
duplicate_messages_threshold_reached
duplicate_groups_threshold_reached
max_copies_threshold_reached
```

The threshold booleans refer to the threshold band that selected the final level:

- for `Critical`, they show critical numeric dimensions reached;
- for `Warning`, they show warning numeric dimensions reached;
- for `Notice` and `Clear`, all threshold flags are false.

`requires_manual_review()` is true only for identity conflict. A numeric `Critical` result alone does not authorize destructive handling.

## Privacy boundary

Input is already the count-only `DlqDuplicateSummary`. The evaluation does not expose:

- broker address;
- stream, topic, partition, or offset;
- message UUID;
- payload or payload digest;
- receipt identity or state;
- producer identity;
- credentials;
- timestamps;
- raw threshold values.

The policy adds no serialization implementation. A caller that serializes or exports the evaluation must preserve the same identifier-free boundary.

## Policy boundary

This module does not choose or implement:

- notification destination;
- paging versus ticketing;
- suppression, deduplication, or cooldown of alerts;
- scan cadence;
- threshold storage or tenant overrides;
- readiness or liveness behavior;
- Profiles authorization;
- acknowledgement, delete, purge, replay, or retry;
- broker configuration changes;
- poison receipt claim or state transitions.

Those concerns require separate owner contracts and operational review.

## Relationship to the scanner

`IggyDlqDuplicateScanner` returns `DlqDuplicateSummary`. A caller may pass that summary to this policy, but the scanner does not own thresholds and the policy does not own the scanner.

## Latest-value runtime composition

`DlqDuplicateAlertRuntimePublisher` now provides the next transport-neutral composition boundary:

```text
DlqDuplicateSummary
  -> DlqDuplicateAlertPolicy::evaluate
  -> DlqDuplicateAlertRuntimeSnapshot
  -> read-only subscribers
```

The runtime is defined in `dlq_duplicate_alert_runtime.rs` and documented in `dlq-duplicate-alert-runtime.md`.

It is deliberately separate from the pure policy:

- one single-writer publisher owns generation and the validated policy;
- the initial snapshot is unavailable with no evaluation;
- successful publication evaluates a summary and replaces the latest value;
- unavailable publication clears the prior evaluation;
- subscribers can only read or await changes;
- no server worker, metric, health check, notification route, or persistence is selected.

The watch channel is latest-value state, not an event log. Slow subscribers are not promised every intermediate generation.

## Relationship to Profiles

No alert level, threshold flag, runtime availability, generation, duplicate count, identity-conflict signal, scan result, or retained evidence may authorize profile visibility, follower access, block/mute behavior, or presentation.

Profiles continues to consume authoritative owner-port results. This policy and runtime composition are operational observability only.

## Source verification

Machine contracts:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-policy-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-runtime-source.json
```

Static verifiers:

```bash
node scripts/verify/verify-iggy-dlq-duplicate-alert-policy.mjs
node scripts/verify/verify-iggy-dlq-duplicate-alert-runtime.mjs
```

Focused policy tests define invalid configuration, clear/notice behavior, warning dimensions, critical numeric precedence, and conflict-critical manual escalation. Runtime tests define initial unavailability, latest-value publication, stale-evaluation clearing, independent subscribers, and bounded publisher closure.

No tests, Cargo commands, formatters, source verifiers, external Iggy connections, server observers, telemetry registration, alert delivery, or retained capture were run while authoring these slices.

## Remaining work

1. integrate an explicitly owned server observer;
2. project the runtime snapshot into reviewed telemetry and health contracts;
3. define alert delivery, cooldown, and suppression outside the policy/runtime;
4. retain privacy-safe policy/runtime integration evidence;
5. define destructive reconciliation as a separate authorized workflow;
6. compare aggregate receipt and duplicate trends without exporting identifiers.
