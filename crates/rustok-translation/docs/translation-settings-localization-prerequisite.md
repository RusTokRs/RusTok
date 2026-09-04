---
id: doc://crates/rustok-translation/docs/translation-settings-localization-prerequisite.md
kind: implementation_handoff
language: en
status: in_progress
last_reviewed: 2026-09-04
---

# Translation Settings localization prerequisite

Status: **owner persistence/read/source/progress, stable identity/descriptors/revisions, and neutral validate/apply command mapping source-ready / runtime provider registration open**

Base reviewed before this slice: `main@44874370cc3313a7bf77c8cf76182b75801858ec`.

## Existing owner foundation

The Settings owner boundary remains layered outside Translation:

- #3825 typed stable localized field IDs, string-leaf eligibility and sensitivity fences;
- #3831 parallel exact-locale storage, exact reads, per-field target CAS, shared owner CAS, and replay-safe exact apply;
- #3832 content-free `change_seq` repair evidence;
- #3833 explicit source-locale provenance bound to the latest `base_projection` revision;
- #3834 bounded owner change reads plus stable exact-locale snapshot/progress facts;
- #3835 stable neutral resource and field identities;
- #3836 conservative field descriptors;
- #3837 opaque resource/source/target revision mapping while retaining per-field target revisions.

Language-neutral Settings stay in `tenant_modules.settings`. Localized copy, source-locale provenance, repair evidence, exact progress, owner revisions, and row target revisions remain owner data. Runtime fallback is not exact coverage.

## What this slice adds: neutral validate/apply mapping

`rustok-modules-translation` still contains no database access and still does not register a `TranslationTargetProvider`. This slice proves the pure mapping needed before runtime execution may be wired.

### Patch validation

`StaticSettingsTranslationIdentity::validate_patch_against_snapshot` validates one neutral `TranslationPatchRequest` against a stable `StaticSettingsExactLocaleSnapshot`.

It checks:

- the neutral patch contract itself, including non-empty fields, unique field keys, proposal identity, and approval receipt identity;
- exact resource identity;
- exact source and target locale equality with the owner snapshot;
- opaque resource/source/target revision preconditions from #3837;
- every patched field is a current source-present owner-admitted field;
- every patched field carries the current SHA-256 source hash.

Expected drift is returned as structured `TranslationPatchValidation` issues such as `resource_revision_conflict`, `source_revision_conflict`, `target_revision_conflict`, `source_hash_conflict`, or `field_not_supported`. No owner command is prepared when validation is rejected.

Owner value/schema validation is intentionally not duplicated here. Concrete min/max and other localized-value rules still belong to `StaticSettingsLocalizationService::apply_exact`.

### Deterministic owner apply plan

`StaticSettingsTranslationIdentity::prepare_apply_plan` converts an accepted neutral patch into `StaticLocalizedSettingApplyCommand` values without executing them.

The plan is deterministic:

1. patch fields are sorted by stable `FieldKey`;
2. the first command uses the stable snapshot's shared owner revision;
3. each following command expects the previous command to have advanced the owner revision by exactly one;
4. each command uses that field's current exact `target_revision`, or `0` when no exact target row exists;
5. the plan exposes the final expected owner revision after all steps.

This preserves both levels of CAS: the aggregate target digest is checked before planning, while each owner write still carries its actual per-field target-row revision and sequential shared owner revision.

### Per-step idempotency

One provider patch may update multiple Settings fields, but owner `apply_exact` stores one durable receipt per field payload. Reusing the same owner idempotency UUID for different field/value payloads would therefore be incorrect.

The mapper derives a deterministic non-nil UUID per sorted field step from:

- a versioned namespace;
- the caller's base owner operation UUID;
- module slug;
- exact target locale;
- field key;
- deterministic step index.

Actor identity, tenant, trace ID, and correlation ID are preserved. The mapper rejects a command context whose tenant does not equal the exact owner snapshot tenant.

The derived key intentionally makes replay of the same prepared provider operation produce the same owner step keys, while different fields in that operation cannot collide with one another.

## What is still not proven

This slice does **not** execute the prepared commands. Runtime provider wiring must still prove the orchestration around those commands, including failure/replay behavior if a multi-field sequence is interrupted after some owner steps have committed.

That registration slice must also expose the read/list/progress/change capabilities from existing owner contracts and register the provider through the neutral target registry. It must not add direct Settings SQL to Translation.

## Remaining provider work

Only the runtime registration/execution slice remains:

1. implement the actual Settings `TranslationTargetProvider` using public owner services and these proven identity/descriptor/revision/validation/apply-plan contracts;
2. define replay-safe provider-level orchestration for multi-field execution without bypassing the per-field owner receipts;
3. register the provider only after that runtime mapping is source-proven.

## Forbidden shortcuts

Do not store localized values in base Settings JSON, count fallback as exact coverage, localize sensitivity-fenced paths, read Settings tables directly from the Translation adapter, bypass shared owner CAS, replace per-field target CAS with the aggregate digest, reuse one owner idempotency key for multiple field payloads, apply fields in caller-supplied nondeterministic order, prepare commands for a mismatched tenant context, weaken owner schema validation, or treat pure command planning as runtime execution evidence.

## Scope

This slice changes only the persistence-free Settings Translation adapter's neutral validation/apply command mapping plus synchronized source evidence/handoff/verifier and the small UUID dependency needed for deterministic owner step keys. It does not change migrations, owner persistence, fallback, provider runtime execution, or provider registration.
