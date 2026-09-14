# Alloy Script Presentation Translation Plan

## Status

This document defines the owner boundary for Alloy Translation integration. It is
intentionally narrower than the legacy `alloy_control_copy` parity row.

The current Alloy `Script` aggregate mixes human-facing prose with runtime and
security-sensitive state. Registering that aggregate as a Translation target
would therefore be incorrect.

ALLOY-TR-1 through ALLOY-TR-4 are complete. The narrow
`alloy/script_presentation` target is registered in production composition over
the owner-local presentation/change plane. It exposes localized `description`
only, requires `scripts.manage` for both read and apply, keeps AI export disabled,
and delegates writes to the durable Alloy owner apply path. Exact committed
replay is admitted from the owner receipt before newer live revisions are read,
while new writes still pass the Translation snapshot/hash validation and owner
resource/source/target CAS. Operational Script writes remain outside the change
plane. ALLOY-TR-5 now has a retained repository-hosted PostgreSQL evidence harness
and exact-head workflow. Pilot promotion remains blocked until that focused
workflow is run green and its reviewed post-merge evidence is retained.

## Audited owner boundary

The canonical owner is Alloy and the stable resource identity is `script_id`
inside a tenant. The current operational `Script.name` is **not** translatable:
it is a tenant-unique runtime/lookup identity and is used by owner storage,
cache invalidation and execution paths.

The following state must remain outside Translation:

- `name` while it is the operational tenant-unique script key;
- Rhai workspace/source, tests, fixtures and generated files;
- trigger configuration and API/event routing;
- permissions, `run_as_system`, capability grants and sandbox policy;
- lifecycle/status, revisions, review/test state and execution evidence;
- source provenance, parent release lineage and error state;
- any prompt/template/code content that can alter execution semantics.

The existing `description` is human-facing presentation prose and is the first
localized Translation field. A future user-facing display name may also be
localized, but it must be introduced as a distinct `display_name`; it must not
reuse or translate the operational `name` field.

## Target contract

The registered Translation resource is:

- owner slug: `alloy`;
- resource kind: `script_presentation`;
- stable identity: tenant + `script_id`;
- fields: `description` only;
- optional future field: `display_name` after an explicit owner-model addition;
- AI export: forbidden until a separate policy explicitly admits Alloy script
  presentation copy;
- authorization floor: the canonical Alloy owner permission (`scripts.manage`)
  for both read and apply operations;
- capabilities: list resources, exact read, aggregate progress, patch validation,
  durable patch apply and frozen ChangeCursor.

The provider does not expose executable source, templates, prompts, operational
`name`, permissions or runtime configuration. Its display label is derived only
from stable Script identity so inventory cannot leak operational Script names as
presentation copy.

## Provenance and locale rules

All new presentation writes carry an explicit normalized source locale. The
owner does not infer a tenant default, installation locale or English for
legacy rows.

Legacy inline descriptions whose original locale is not known retain truthful
unknown provenance as storage-only `und`. The Translation target never promotes
that storage marker to a concrete `TenantLocale`; only concrete exact locale
rows participate in Translation inventory and snapshots.

## Owner storage foundation

Alloy owns one localized row per `(tenant_id, script_id, locale)` containing
presentation fields and copy revision metadata.

The owner foundation provides:

1. exact tenant + script ownership checks;
2. normalized locale keys;
3. source-locale provenance for canonical authoring copy;
4. a copy-only semantic revision that changes only when localized presentation
   content changes;
5. atomic compare-and-swap writes for one locale;
6. durable idempotent apply receipts scoped to the presentation resource;
7. lifecycle behavior that follows canonical Script owner commands rather than
   test-only SQL;
8. a durable translation-change journal with stable aggregate sequencing;
9. exact aggregate progress and a frozen-window ChangeCursor.

The existing whole-script `version` remains the owner command revision. It is
not reused as the Translation copy revision because workspace, trigger,
permission and lifecycle mutations can advance it without changing localizable
copy.

## Durable change-plane contract

The presentation resource revision is a separate monotonic aggregate revision
owned by Alloy. PostgreSQL records at most one externally visible active change
per `(tenant_id, script_id)` in one database transaction, even when several
locale rows change together. Existing presentation rows are backfilled into
resource state without inventing historical change events.

Only semantic presentation row inserts or updates advance the active resource
revision. Metadata-only row rewrites and operational `scripts` mutations do not.
Canonical hard delete of the owning Script records one final `deleted` tombstone
before presentation rows cascade and removes live resource state in the same
transaction.

Apply idempotency is tenant-scoped and durable. A receipt binds the idempotency
key to Script identity, exact source/target locales, workflow evidence, request
fingerprint, expected revisions and requested copy. At the provider boundary,
an exact committed receipt is checked before consulting current presentation
state; this preserves replay even after newer owner changes. A mismatched key or
request fingerprint fails closed. New applies then pass current snapshot/hash
validation before entering the owner transaction, whose own receipt admission
and resource/source/target CAS remain authoritative.

Progress uses exact source-locale inventory and treats `description` as the one
optional unit. The owner brackets one aggregate progress read with journal
high-water reads and retries a bounded number of times if they differ. Change
pages use opaque `v1:<through>:<after>` cursors: the first page freezes the
current high-water, intermediate pages preserve it, and a tail cursor starts the
next polling window from the previous checkpoint.

## Canonical authoring integration

`AlloyAuthoringService` remains the canonical owner write boundary for Script
create/update transports. Translation writes enter the same presentation owner
CAS through `SeaOrmScriptPresentationTranslationStore`; the provider never
creates a second presentation writer.

The implemented sequence is:

- explicit presentation/source-locale input on owner commands;
- canonical presentation copy persisted in the same owner transaction as the
  corresponding Script mutation;
- operational `name` semantics unchanged;
- description reads remain owner-local and locale-aware;
- Translation exact read/list/progress/change capabilities read only owner-owned
  presentation/resource state;
- Translation apply delegates to the durable owner receipt + CAS path.

## Delivery slices

### ALLOY-TR-1 — boundary and parity — complete

- classify legacy `alloy_control_copy` as an excluded broad aggregate;
- track the narrow `alloy/script_presentation` owner target separately;
- record the audited source files as evidence;
- keep provider status `not_registered`.

### ALLOY-TR-2 — owner presentation storage — complete

- add localized presentation storage and source-locale provenance;
- integrate create/update through `AlloyAuthoringService`;
- add copy-only semantic revisions and owner-level CAS;
- do not register a Translation provider.

### ALLOY-TR-3 — durable change plane — complete

- add idempotent apply receipts;
- add semantic change journal, lifecycle tombstones, aggregate progress and
  frozen ChangeCursor semantics;
- keep non-copy Script mutations outside presentation changes.

### ALLOY-TR-4 — provider composition — complete

- implement narrow `alloy/script_presentation` list/read/validate/apply/progress/
  ChangeCursor target behavior;
- expose only optional tenant-private `description`, with AI export forbidden;
- require `scripts.manage` for read and apply;
- preserve durable exact replay before live snapshot validation;
- register the provider in canonical production host composition.

### ALLOY-TR-5 — retained PostgreSQL evidence — harness complete, green run pending

- `apps/server/tests/alloy_script_presentation_translation_target_postgres.rs`
  retains migration/backfill, tenant-isolation, canonical owner authoring,
  operational-only no-copy churn, multi-replica CAS, exact durable replay after
  later owner changes, aggregate progress, frozen-window recovery and canonical
  hard-delete tombstone evidence;
- the evidence proves `scripts.manage` policy, tenant-private/AI-forbidden field
  metadata and that storage-only `und` provenance is never advertised as a
  concrete exact locale or fabricated historical change;
- `.github/workflows/alloy-script-presentation-translation-target-postgres.yml`
  checks out and asserts the exact PR/main SHA, performs focused formatting,
  compiles the locked focused test and runs it against PostgreSQL 16;
- run the focused workflow green and retain reviewed post-merge evidence before
  considering pilot promotion.

## Non-goals

This track does not localize source code, Rhai strings, prompts, templates,
review/test evidence, trigger/API names, permissions, runtime errors or other
operational/security-sensitive state. If any of those need localization later,
they require their own typed owner contract rather than widening
`script_presentation`.
