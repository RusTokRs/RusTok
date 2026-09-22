# RBAC Translation Presentation Plan

## Status

This document defines the owner boundary for RBAC Translation integration. It is
intentionally narrower than the existing broad `rbac_control_copy` parity row.

RBAC authorization identity is not presentation copy. Canonical role slugs,
permission keys, resources/actions, relation state, grants, scopes, release and
installation identity, continuity fingerprints, durable invalidation generations,
receipts and events participate in authorization semantics and must never become
translatable values.

RBAC-TR-1 and RBAC-TR-2 are complete. RBAC-TR-3 is now in progress: the canonical
GraphQL role read resolves human-facing role names from the owner-localized plane
using exact request locale first and explicit `en` source copy second. A temporary
legacy seed fallback remains only for tenants that do not yet have built-in owner
presentation rows. Built-in seed presentation and the native admin bootstrap still
use the legacy inline/derived path, so the RBAC Translation provider remains
blocked and unregistered until those remaining TR-3 write/read paths are cut over.
Existing artifact-permission translations remain a separate RBAC-owned
release-governance localization plane and are not a second tenant Translation
writer.

## Audited authorization identity boundary

The following values are authorization identity or evidence and are never
Translation fields:

- canonical `UserRole` slugs such as `super_admin`, `admin`, `manager`, and
  `customer`;
- permission keys and their resource/action identity;
- role/permission relation rows and user-role assignments;
- tenant/platform scope identity and admitted installation identity;
- module/release digests and immutable artifact-permission keys;
- authorization/continuity fingerprints;
- durable invalidation generations, assignment receipts and integration events;
- policy facts used to evaluate grants, hierarchy, continuity or revocation.

Presentation updates must not change any of those values, alter permission
membership, advance authorization identity, or create a new route around the
canonical RBAC mutation policy.

## Owner-local localized presentation storage

The built-in catalog separates stable role/permission slugs from human-readable
copy, while legacy presentation is still inline or derived:

- built-in role display names are hard-coded from `UserRole`;
- permission display names are synthesized from the permission slug;
- future tenant-authored role/permission presentation must carry explicit locale
  provenance and remain identity-bound to the canonical role/permission key.

RBAC-TR-2 adds `rbac_localized_presentations`, keyed by tenant, admitted resource
kind (`role` or `permission`), stable canonical resource key, and normalized exact
stored locale. The owner store exposes concrete `RuntimeLocale` at canonical
source-write edges, so newly authored copy cannot silently use storage-only
`und`. Reads retain `StoredLocale` support so a future migration of genuinely
persisted legacy copy with unknown provenance can remain truthful under `und`
without guessing English, a tenant default, request locale, or deployment locale.

There is no current persisted owner role/permission presentation data to backfill:
the legacy built-in copy is computed inline. RBAC-TR-2 therefore deliberately does
not fabricate `und` rows from derived strings. RBAC-TR-3 will seed canonical
presentation through explicit-locale owner data instead.

Presentation copy has its own `copy_revision`. Idempotent replays do not advance
that revision, and semantic copy changes use compare-and-set. Authorization and
invalidation revisions/generations are not reused by this plane.

Owner writes are identity-bound: the current store admits only resource keys that
exist in `BuiltinTenantRbacCatalog`. Extending the owner to tenant-authored roles
or permissions requires first extending canonical RBAC identity ownership; copy
storage must not create orphan authorization identity.

Authorization and mutation transports continue to use stable identity. Locale-
aware admin reads may resolve presentation copy, but policy writes, assignments,
permission checks and events stay identity-bound.

## Existing artifact-permission localization is a separate plane

`RbacArtifactPermissionCatalog` already persists language-neutral artifact
permission identity separately from normalized localized `label` and
`description` rows. Release-localized copy is projected into scoped owner
translation rows without participating in the canonical authorization
fingerprint.

That localization belongs to RBAC artifact/release governance:

- release/module/permission-key identity is immutable authorization metadata;
- localized labels/descriptions are admitted from artifact release metadata;
- display-only changes remain outside authorization fingerprint identity;
- tenant Translation must not create a competing mutable writer for these rows.

If artifact-permission localization ever needs Translation workflow authoring,
it requires an explicit platform/release-scoped ownership contract. The current
tenant Translation target track does not silently absorb it.

## Proposed narrow Translation contract

No RBAC Translation provider is registered through RBAC-TR-3 while the canonical
cutover remains incomplete. After the canonical write/read cutover, the candidate
tenant Translation surface is presentation-only role/permission copy:

- owner slug: `rbac`;
- stable resource identity: canonical role or permission identity inside its
  admitted tenant scope;
- candidate fields: role display name/description and permission display
  label/description only where the owner explicitly classifies them as mutable
  tenant presentation;
- authorization identity fields: excluded;
- AI export: forbidden unless a future explicit RBAC policy safely admits it;
- control-plane admission: must preserve the canonical direct-session/tenant and
  RBAC management permission floors rather than adding a Translation bypass.

Resource kinds and copy-only revision semantics are now owner-defined by TR-2.
The final field/write contract remains intentionally deferred until RBAC-TR-3
moves every canonical read and write onto this owner plane.

## Delivery slices

### RBAC-TR-1 — boundary and parity — complete

- distinguish immutable authorization identity from human-facing presentation;
- classify existing artifact-permission translations as separate release-governance
  localization rather than tenant Translation data;
- keep `rbac_control_copy` blocked and its provider unregistered;
- retain AI export as fail-closed/forbidden while the owner cutover is incomplete.

### RBAC-TR-2 — owner localized presentation storage — complete

- owner-local normalized presentation rows are keyed by tenant, admitted stable
  role/permission identity, and exact locale;
- new presentation writes require explicit concrete `RuntimeLocale` provenance;
- storage can represent truthful legacy `und`, but no `und` backfill is invented
  because current legacy role/permission copy is computed rather than persisted;
- copy-only compare-and-set revisions are independent from authorization and
  invalidation generations;
- owner writes reject unknown role/permission identity;
- artifact-permission release translations stay on their existing governance
  plane.

### RBAC-TR-3 — canonical write/read cutover — in progress

Completed in the current API-read slice:

- GraphQL role presentation resolves from owner-localized exact request locale;
- missing exact locale falls back only to the explicit owner source locale `en`;
- role slug and permission membership remain stable authorization identity and are
  not derived from localized copy;
- tenants without seeded owner rows retain a narrow temporary legacy seed fallback
  so this read migration is deployable before the write-side seed cutover.

Remaining before RBAC-TR-3 can be marked complete:

- route built-in seed presentation through explicit-locale owner data, remove the
  temporary GraphQL seed fallback, and prove idempotent bootstrap does not
  overwrite tenant-edited copy;
- make any tenant-authored presentation commands locale-aware and identity-bound;
- migrate the native admin bootstrap presentation read to the same owner-aware
  service/host contract instead of its current inline labels;
- prove display-only edits cannot change grants, fingerprints, assignments or
  authorization events;
- retire all remaining legacy inline/derived presentation only after every
  canonical read and write uses the localized owner plane.

### RBAC-TR-4 — narrow Translation provider

- decide and register only owner-approved mutable presentation resources after
  RBAC-TR-2/3 are complete;
- expose no authorization identity as a translatable field;
- preserve direct-session/tenant and RBAC management permission floors;
- add revision-safe validation/apply, durable replay, aggregate progress and a
  bounded change contract owned by RBAC if the resulting surface requires them;
- do not duplicate artifact-permission release-localization writes.

### RBAC-TR-5 — retained PostgreSQL evidence

- retain migration/backfill and exact-locale/source-provenance evidence;
- prove tenant isolation and fail-closed unknown provenance;
- prove presentation edits leave canonical role/permission identity, grants,
  fingerprints and authorization generations unchanged unless a real policy
  mutation separately occurs;
- retain canonical control-plane authorization evidence, including direct-user
  and tenant boundaries;
- prove the existing artifact-permission localization plane is not mutated by
  tenant Translation;
- retain exact-head and reviewed post-merge PostgreSQL evidence before any pilot
  promotion.

## Non-goals

This track does not translate or rewrite role slugs, permission keys,
resource/action identity, assignments, grants, scopes, installation/release
identity, fingerprints, invalidation generations, receipts, policy facts or
events. It also does not turn artifact release-localized permission metadata into
a tenant-editable Translation surface by implication.
