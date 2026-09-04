---
id: doc://crates/rustok-translation/docs/translation-settings-localization-prerequisite.md
kind: implementation_handoff
language: en
status: in_progress
last_reviewed: 2026-09-04
---

# Translation Settings localization prerequisite

Status: **exact-locale owner storage/apply source-ready / change evidence and provider onboarding open**

Base reviewed before this slice: `main@0cd5e9d37dc41334f20c2b347d946ee0055d0199`.

## Correction to the previous prerequisite

PR #3830 correctly identified that #3825 supplied only typed localization metadata and source-value snapshots, but it was too conservative about the existing static Settings write boundary. `ModuleLifecycleDbWriter::update_static_normalized_settings` already uses the shared `StaticTenantLifecycleStore` expected-revision CAS and durable `owner_operation_receipts` idempotency in the same owner transaction as the base `tenant_modules.settings` write.

That existing lifecycle CAS is reused here; it is not duplicated or replaced.

## What this slice adds

Static module Settings now have a separate exact-locale persistence boundary in `rustok-modules`:

- `module_static_localized_settings` stores localized copy outside the language-neutral `tenant_modules.settings` JSON under `(tenant_id, module_slug, field_id, locale)`;
- PostgreSQL storage is tenant-RLS scoped, while SQLite uses the same logical identity for focused tests and development;
- `StaticSettingsLocalizationRegistry` reuses #3825 owner metadata and sensitivity fences, so only declared localized string leaves can reach this path;
- target values retain schema string-length constraints and a hard bounded UTF-8 payload ceiling;
- `source_snapshot` returns deterministic owner-declared source values together with the shared static owner revision and fails closed if that owner revision changes during the read;
- `read_exact` requires canonical `TenantLocale` and never performs runtime fallback;
- `apply_exact` requires both expected static owner revision and expected target-row revision;
- localized apply claims and advances the same `StaticTenantLifecycleStore` aggregate used by base settings/lifecycle writes, so base settings and localized copy cannot cross stale revisions;
- the exact target row and owner revision advance commit together;
- apply uses the shared durable `owner_operation_receipts` contract, binding actor/idempotency/request identity and storing the terminal response in the owner transaction;
- `und` and non-canonical runtime locale spellings are rejected before persistence.

The new storage does not write translations back into base Settings JSON and does not introduce fallback reads.

## Why the Settings Translation gate remains open

This slice deliberately stops before provider registration. Two owner/provider prerequisites remain material:

1. define transactional content-free Settings localization change evidence or a bounded repair cursor so Translation inventory/progress can recover after missed notifications;
2. define the authoritative source-locale policy for static Settings copy before a provider can expose exact source/target contracts.

Only after those are explicit should a Settings Translation target register exact inventory, validation, apply and progress through `rustok-translation-targets`.

## Forbidden shortcuts

Do not store localized values in the base settings JSON, count rendered fallback as exact coverage, localize secret/sensitivity-fenced paths, bypass the shared static lifecycle revision, invent a generic settings event outside an owner transaction, or register a provider without source-locale and repair semantics.

## Scope

This slice changes only the Settings owner persistence foundation plus this Translation handoff/evidence. It does not register a Translation provider, add runtime fallback, change module enablement semantics, touch artifact Settings persistence, or overlap Forum UGC onboarding.
