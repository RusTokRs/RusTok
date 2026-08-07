# Count-only physical DLQ duplicate alert policy

Status: **policy, latest-value runtime, and mode-aware server observer source-complete; runtime execution and retained evidence pending**.

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

No production default is provided. Validation fails closed unless warning values are meaningful and each critical value is not below its warning value.

Invalid configuration returns:

```text
iggy.dlq_duplicate.alert_policy_invalid
```

Equal warning and critical thresholds are valid. Critical evaluation has precedence.

## Alert levels

```text
Clear     no physical duplicate
Notice    duplicates exist below warning thresholds
Warning   at least one warning threshold reached
Critical  at least one critical threshold reached
          OR any identity conflict exists
```

Stable codes:

```text
iggy.dlq_duplicate.alert.clear
iggy.dlq_duplicate.alert.notice
iggy.dlq_duplicate.alert.warning
iggy.dlq_duplicate.alert.critical
```

Identity conflict always has precedence because one deterministic header UUID was observed with different exact payload bytes.

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

The threshold booleans refer to the threshold band that selected the final level. `requires_manual_review()` is true only for identity conflict. A numeric `Critical` result alone does not authorize destructive handling.

The evaluation excludes broker addresses, stream coordinates, message UUIDs, payloads/digests, receipt identities, producer identity, credentials, timestamps, and raw threshold values.

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
- poison receipt state transitions.

## Relationship to the scanner

`IggyDlqDuplicateScanner` returns `DlqDuplicateSummary`. The scanner does not own thresholds and the policy does not own the scanner.

## Latest-value runtime composition

`DlqDuplicateAlertRuntimePublisher` provides the transport-neutral composition boundary:

```text
DlqDuplicateSummary
  -> DlqDuplicateAlertPolicy::evaluate
  -> DlqDuplicateAlertRuntimeSnapshot
  -> read-only subscribers
```

It is deliberately separate from the pure policy:

- one single-writer publisher owns generation and the validated policy;
- initial state is unavailable with no evaluation;
- successful publication replaces the latest value;
- unavailable publication clears the prior evaluation;
- subscribers can only read or await changes;
- no broker, server lifecycle, metric, health check, notification route, or persistence is selected.

## Mode-aware server observer

The host now owns a separate integration that composes the scanner, policy, and runtime without assuming every event profile uses Iggy:

```text
memory        -> not applicable, no Iggy access
outbox        -> not applicable, no Iggy access
outbox_iggy   -> bundled or external read-only observer
```

For non-Iggy profiles, no broker client is opened and alert thresholds are not required. For `outbox_iggy`, all six thresholds remain explicit and the observer uses the exact active transport configuration.

Bundled mode connects to the already-running loopback broker. External mode uses the reviewed address list. The observer never creates a second transport or process, and a missing active Iggy mode fails closed rather than being guessed.

Connection or scan failure publishes unavailable state while event delivery and module projection remain active. Notification delivery, cooldown, suppression, telemetry, health projection, and destructive reconciliation remain separate owners.

See `dlq-duplicate-alert-server-observer.md` for the complete mode and lifecycle contract.

## Relationship to Profiles

No alert level, threshold flag, observer mode, runtime availability, generation, duplicate count, identity-conflict signal, scan result, or retained evidence may authorize profile visibility, follower access, block/mute behavior, or presentation.

Profiles continues to consume authoritative owner-port results. This path is operational observability only.

## Source verification

Machine contracts:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-policy-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-runtime-source.json
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-server-observer-source.json
```

Static verifiers:

```bash
node scripts/verify/verify-iggy-dlq-duplicate-alert-policy.mjs
node scripts/verify/verify-iggy-dlq-duplicate-alert-runtime.mjs
node scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs
```

No tests, Cargo commands, formatters, source verifiers, broker connections, server observers, telemetry registration, alert delivery, or retained capture were run while authoring these slices.

## Remaining work

1. project the runtime snapshot into reviewed telemetry and optional health without readiness coupling;
2. define alert delivery, cooldown, and suppression outside the policy/runtime;
3. retain privacy-safe policy/runtime/server execution evidence;
4. define destructive reconciliation as a separate authorized workflow;
5. compare aggregate receipt and duplicate trends without exporting identifiers.
