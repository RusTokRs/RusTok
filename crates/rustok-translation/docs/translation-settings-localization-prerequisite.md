---
id: doc://crates/rustok-translation/docs/translation-settings-localization-prerequisite.md
kind: implementation_handoff
language: en
status: in_progress
last_reviewed: 2026-09-04
---

# Translation Settings localization prerequisite

Status: **exact-locale owner storage/apply plus transactional repair cursor source-ready / source-locale policy and provider onboarding open**

Base reviewed before this slice: `main@7678e1ee9a7208a8e19663732ebaef93184494e4`.

## Existing owner foundation

PR #3831 established the exact-locale Settings owner boundary on top of the pre-existing static lifecycle aggregate:

- language-neutral Settings remain in `tenant_modules.settings`;
- localized copy lives separately in `module_static_localized_settings` under tenant/module/stable-field/canonical-locale identity;
- exact reads never substitute runtime fallback;
- target-row revision CAS and the shared `StaticTenantLifecycleStore` owner revision guard stale writes;
- owner apply is replayable through the durable operation-receipt ledger;
- #3825 localization metadata and sensitivity fences remain the admission boundary for fields that may enter localized storage.

## What this slice adds

Static Settings now have durable, content-free repair evidence in `module_static_settings_changes`.

The cursor contract is intentionally owner-local and transport-neutral:

- `change_seq` is an append-only database sequence, not an event timestamp or translated-content hash;
- every row is tenant/module scoped and contains no source or translated value;
- `base_projection` rows invalidate the Settings source projection while recording only the next shared owner revision;
- `localized_target` rows identify only stable field ID, canonical locale, shared owner revision and target-row revision;
- one `(tenant_id, module_slug, owner_revision)` change is admitted, matching the single shared static owner mutation boundary;
- the `(tenant_id, module_slug, change_seq)` index provides bounded keyset repair without deriving ordering from opaque timestamps;
- PostgreSQL keeps the journal tenant-RLS scoped;
- SQLite retains the same logical sequence and trigger semantics for focused local verification.

The journal is emitted by database triggers inside the same database transaction as the owner write:

- source/base projection evidence is produced only while the tenant/module static lifecycle aggregate carries an active owner claim; the row records `current_revision + 1`, which is the revision the same transaction must advance to before commit;
- exact localized target insert/update evidence uses the `owner_revision` and target `revision` already written by the localized owner transaction;
- if owner CAS, localized persistence, receipt completion, or transaction commit fails, the corresponding change row rolls back with it.

An initial static override materialization may conservatively emit `base_projection` evidence even when its Settings JSON remains the empty base object. That is safe over-invalidation: repair performs a re-read and never fabricates exact-locale coverage.

## Why the Settings Translation gate remains open

The transactional repair prerequisite is now source-ready, but provider registration still waits on two explicit boundaries:

1. define the authoritative source-locale policy for tenant-module localized Settings copy; `und`, runtime fallback and guessed tenant defaults cannot silently become an authoring source;
2. wire the eventual Settings provider to a bounded owner change reader over `change_seq` and prove exact-locale progress/inventory semantics through `rustok-translation-targets`.

The provider should consume this journal as repair evidence; it must not expose the physical table or let Translation write it directly.

## Forbidden shortcuts

Do not store localized values in the base settings JSON, count rendered fallback as exact coverage, localize secret/sensitivity-fenced paths, bypass the shared static lifecycle revision, include source/target text in the change journal, infer repair ordering from timestamps, or register a provider before source-locale semantics are explicit.

## Scope

This slice adds only the Settings owner repair journal, migration wiring, and synchronized source evidence. It does not register a Translation provider, select a source locale, add runtime fallback, change module enablement semantics, touch artifact Settings persistence, or overlap Forum UGC onboarding.
