---
id: doc://crates/rustok-translation/docs/translation-settings-localization-prerequisite.md
kind: implementation_handoff
language: en
status: in_progress
last_reviewed: 2026-09-05
---

# Translation Settings localization prerequisite

Status: **owner persistence/read/source/progress, stable identity/descriptors/revisions, neutral validate/apply mapping, and runtime provider registration/execution source-proven**

Base reviewed after runtime completion: `main@534ea65984c5aca3884c4a86363bcd95a207081c`.

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

## Neutral validate/apply mapping

`rustok-modules-translation` contains no database access and does not own Settings persistence. It provides the pure mapping used by the registered runtime provider.

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

## Runtime provider completion

PR #3840 registers the Settings target through the neutral Translation target registry and completes the runtime execution slice without adding direct Settings SQL to Translation.

The server-owned `StaticSettingsTranslationTargetProvider` now:

- resolves admitted package-localization metadata through the host Settings localization registry;
- exposes `list_resources`, exact `read_resource`, aggregate `read_progress`, bounded `read_changes`, `validate_patch`, and `apply_patch` through public owner services;
- preserves the neutral owner/resource identity contract `modules/static_settings`;
- enforces `settings:read` and `settings:update` authorization floors at the provider boundary;
- validates exact source/target locale and revision/source-hash preconditions through the persistence-free adapter before owner execution;
- wraps multi-field execution in provider-level durable idempotency while retaining deterministic per-field owner receipts;
- fails closed when an interrupted multi-field sequence has advanced owner state, requiring a fresh read/proposal instead of bypassing shared or per-field CAS;
- uses one opaque change-cursor contract for both progress checkpoints and subsequent change polling, with a stable high-water tail checkpoint encoded in the same cursor domain accepted by `read_changes`.

Runtime registration is asserted by server composition tests; the provider remains a consumer of owner contracts rather than a second Settings persistence implementation.

## Validation evidence

The runtime-provider slice was squash-merged by #3840 as `main@534ea65984c5aca3884c4a86363bcd95a207081c` from exact feature head `92cda03363dc40c1f2cac35f5b464078a82e1022`.

Focused evidence on that exact head:

- Translation Runtime Composition Evidence `33952057144`: successful, including exact-SHA checkout, Translation boundary verification, Translation-only GraphQL `StorageRuntime` composition evidence, and authenticated production application-router native evidence;
- Migration harness approval `33952055814`: successful.

Repository Ruleset Contract `33952055715` failed before evaluating #3840 because its base `main@258b96f63626442ab2ccb5669cccad2446f17f14` failed the repository source-policy self-test. Recovery PR #3842 restored the source ruleset payload as `c44dc21bc9bf29d6039e9cbf18baa999e25f87a4`. That repository-governance incident is not a Settings runtime-provider failure and does not reopen this prerequisite.

## Remaining work outside this prerequisite

No Settings `TranslationTargetProvider` implementation or registration work remains in this handoff.

Any broader Translation rollout evidence that depends on a live production environment, repository administration, or future product policy belongs to those owning plans. Future Settings Translation changes must continue to use the public owner services and neutral target registry rather than reopening direct persistence ownership in Translation.

## Forbidden shortcuts

Do not store localized values in base Settings JSON, count fallback as exact coverage, localize sensitivity-fenced paths, read Settings tables directly from the Translation adapter/provider, bypass shared owner CAS, replace per-field target CAS with the aggregate digest, reuse one owner idempotency key for multiple field payloads, apply fields in caller-supplied nondeterministic order, prepare commands for a mismatched tenant context, weaken owner schema validation, or treat fallback as source/target coverage.

## Scope

This handoff records the completed Settings localization prerequisite chain through the registered runtime provider. It covers the owner contracts, persistence-free neutral identity/descriptor/revision/validation/apply mapping, runtime list/read/progress/change/validate/apply orchestration, and focused evidence described above. It does not move Settings persistence into Translation, change owner migrations or fallback semantics, or claim unrelated live-environment/repository-administration evidence.
