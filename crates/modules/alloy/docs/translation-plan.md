# Alloy Script Presentation Translation Plan

## Status

This document defines the owner boundary that must exist before Alloy can register
any Translation target. It is intentionally narrower than the legacy
`alloy_control_copy` parity row.

The current Alloy `Script` aggregate mixes human-facing prose with runtime and
security-sensitive state. Registering that aggregate as a Translation target
would therefore be incorrect.

ALLOY-TR-1 and ALLOY-TR-2 are complete. ALLOY-TR-3 now has an owner-local durable
change plane: presentation apply receipts, semantic aggregate revisions and
journal rows, canonical Script hard-delete tombstones, exact-locale aggregate
progress, and bounded frozen ChangeCursor semantics. Operational Script writes
remain outside that plane because only `alloy_script_presentations` semantic
copy changes advance the active resource revision. Provider registration remains
blocked until ALLOY-TR-4; retained PostgreSQL execution evidence remains
ALLOY-TR-5.

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
candidate for localized storage. A future user-facing display name may also be
localized, but it must be introduced as a distinct `display_name`; it must not
reuse or translate the operational `name` field.

## Target contract

The future Translation resource is:

- owner slug: `alloy`;
- resource kind: `script_presentation`;
- stable identity: tenant + `script_id`;
- initial fields: `description` only;
- optional future field: `display_name` after an explicit owner-model addition;
- AI export: forbidden until a separate policy explicitly admits Alloy script
  presentation copy;
- authorization floor: the canonical Alloy owner permission (`scripts.manage`)
  for both read and apply operations.

No Translation provider may expose executable source, templates, prompts,
permissions or runtime configuration as fields of this resource.

## Provenance and locale rules

All new presentation writes must carry an explicit normalized source locale.
The owner must not infer a tenant default, installation locale or English for
legacy rows.

Legacy inline descriptions whose original locale is not known must retain
truthful unknown provenance. Migration code may represent this as an explicit
unknown/undetermined locale only if that representation is accepted by the
shared locale contract; it must never guess a concrete locale.

## Owner storage foundation

Before provider registration, Alloy needs parallel owner-local storage for
presentation copy. The intended shape is one localized row per
`(tenant_id, script_id, locale)` containing only presentation fields and their
copy revision metadata.

The owner foundation must provide:

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
9. aggregate progress and a frozen-window ChangeCursor before provider
   registration.

The existing whole-script `version` remains the owner command revision. It must
not be reused as the Translation copy revision because workspace, trigger,
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
fingerprint, expected revisions and requested copy. Exact replay returns the
committed receipt before checking newer live revisions; mismatched key reuse
fails closed.

Progress uses exact source-locale inventory and treats `description` as the one
optional unit. The owner brackets one aggregate progress read with journal
high-water reads and retries a bounded number of times if they differ. Change
pages use opaque `v1:<through>:<after>` cursors: the first page freezes the
current high-water, intermediate pages preserve it, and a tail cursor starts the
next polling window from the previous checkpoint.

## Canonical authoring integration

`AlloyAuthoringService` remains the write boundary. Create/update transports
must not write localized rows directly.

The migration sequence is:

- introduce explicit presentation/source-locale input on owner commands;
- persist canonical presentation copy in the same owner transaction as the
  corresponding Script mutation;
- keep operational `name` semantics unchanged;
- move description reads to locale-aware owner presentation resolution where a
  locale is requested, with a truthful fallback to the canonical owner copy;
- only after the owner path is durable, expose Translation read/apply ports.

## Delivery slices

### ALLOY-TR-1 — boundary and parity

- classify legacy `alloy_control_copy` as an excluded broad aggregate;
- track the narrow `alloy/script_presentation` owner target separately;
- record the audited source files as evidence;
- keep provider status `not_registered`.

### ALLOY-TR-2 — owner presentation storage

- add localized presentation storage and source-locale provenance;
- integrate create/update through `AlloyAuthoringService`;
- add copy-only semantic revisions and owner-level CAS;
- do not register a Translation provider.

### ALLOY-TR-3 — durable change plane

- add idempotent apply receipts;
- add semantic change journal, lifecycle tombstones, aggregate progress and
  frozen ChangeCursor semantics;
- prove non-copy Script mutations do not create presentation changes.

### ALLOY-TR-4 — provider composition

- implement the narrow `alloy/script_presentation` Translation target;
- register it in production composition with `scripts.manage` policy;
- expose only admitted presentation fields.

### ALLOY-TR-5 — retained PostgreSQL evidence

- retain exact-head PostgreSQL evidence for migration/backfill, tenant
  isolation, canonical create/update, concurrent CAS, idempotent replay,
  lifecycle, progress and ChangeCursor recovery;
- only then consider pilot promotion.

## Non-goals

This track does not localize source code, Rhai strings, prompts, templates,
review/test evidence, trigger/API names, permissions, runtime errors or other
operational/security-sensitive state. If any of those need localization later,
they require their own typed owner contract rather than widening
`script_presentation`.
