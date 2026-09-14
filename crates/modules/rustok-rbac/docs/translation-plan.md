# RBAC Translation Presentation Plan

## Status

This document defines the owner boundary for RBAC Translation integration. It is
intentionally narrower than the existing broad `rbac_control_copy` parity row.

RBAC authorization identity is not presentation copy. Canonical role slugs,
permission keys, resources/actions, relation state, grants, scopes, release and
installation identity, continuity fingerprints, durable invalidation generations,
receipts and events participate in authorization semantics and must never become
translatable values.

RBAC-TR-1 is complete with this boundary audit. The owner still has inline or
computed presentation copy for the built-in role/permission catalog, so the
RBAC Translation provider remains blocked and unregistered until owner-local
localized presentation storage and canonical locale-aware writes replace that
copy. Existing artifact-permission translations are a separate RBAC-owned
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

## Presentation copy requiring owner-local localized storage

The built-in catalog already separates stable role/permission slugs from
human-readable display copy, but that copy is currently inline or derived:

- built-in role display names are hard-coded from `UserRole`;
- permission display names are currently synthesized from the permission slug;
- legacy role names/descriptions and permission descriptions remain presentation
  concerns identified by the multilingual storage audit;
- future tenant-authored role/permission presentation must carry explicit locale
  provenance and remain identity-bound to the canonical role/permission key.

The owner cutover therefore needs localized role/permission presentation rows
keyed by stable identity plus normalized exact locale. New writes must require an
explicit source/effective locale. Legacy copy whose original locale cannot be
proven retains truthful storage-only `und`; RBAC must not infer English, a tenant
default, request locale, or deployment locale during backfill.

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

No RBAC Translation provider is registered by RBAC-TR-1. After the owner storage
and write cutover, the candidate tenant Translation surface is presentation-only
role/permission copy:

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

The exact resource kinds, revision model and field set are intentionally deferred
until owner-local storage and canonical writes exist. Translation must not define
those contracts ahead of the owner.

## Delivery slices

### RBAC-TR-1 — boundary and parity — complete

- distinguish immutable authorization identity from human-facing presentation;
- classify existing artifact-permission translations as separate release-governance
  localization rather than tenant Translation data;
- keep `rbac_control_copy` blocked and its provider unregistered;
- retain AI export as fail-closed/forbidden while the owner cutover is incomplete.

### RBAC-TR-2 — owner localized presentation storage

- add owner-local normalized role/permission presentation rows keyed by stable
  identity plus exact locale;
- carry explicit source-locale provenance for new presentation writes;
- preserve unknown legacy provenance as storage-only `und` without guessing;
- define copy-only revisions that do not reuse authorization/invalidation
  revisions;
- keep artifact-permission release translations on their existing governance
  plane.

### RBAC-TR-3 — canonical write/read cutover

- route built-in seed presentation through explicit-locale owner data;
- make tenant-authored presentation commands locale-aware and identity-bound;
- migrate admin/API presentation reads to owner locale-aware services;
- prove display-only edits cannot change grants, fingerprints, assignments or
  authorization events;
- retire legacy inline/derived presentation only after all canonical reads and
  writes use the localized owner plane.

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
