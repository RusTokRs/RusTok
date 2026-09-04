---
id: doc://crates/rustok-translation/docs/translation-settings-localization-prerequisite.md
kind: implementation_handoff
language: en
status: in_progress
last_reviewed: 2026-09-04
---

# Translation Settings localization prerequisite

Status: **typed metadata complete / owner persistence and provider onboarding open**

Reviewed against `main@bd732fbf80c9169af2d86888a7c44cfc5b9486e8` (tree `f80eaf1a64bd1367630cb841129024a86b9f5f92`).

## What current main already proves

PR #3825 established the first Settings localization prerequisite in `rustok-modules` without changing the public `ModuleSettingSpec` Rust shape:

- owners can declare stable localized field IDs mapped to schema paths;
- only string leaves are eligible;
- enum/options and array-item targets fail closed;
- owner-declared sensitive paths fence that node and every descendant;
- localized field inventory is deterministic;
- normalized language-neutral settings can produce a deterministic source-value snapshot keyed by stable field ID.

This is enough to classify and discover candidate copy. It is not an exact-locale Settings owner yet.

## Why the Settings gate remains open

Current source does not provide parallel tenant/module/field/locale persistence for localized values, a revisioned exact-locale row contract, owner apply idempotency receipts, or transactional change evidence suitable for durable Translation cursor repair. No Settings Translation provider should be registered until those owner guarantees exist.

The central Translation P0 Settings exit condition therefore remains materially open even though its older wording understates the metadata work already merged.

## Required next owner slice

Implement the persistence boundary in the Settings owner before Translation provider wiring:

1. assign one canonical Settings owner for tenant-module localized copy;
2. store localized values outside the language-neutral settings JSON under a tenant/module/stable-field/`TenantLocale` identity;
3. expose exact-locale reads that never substitute runtime fallback;
4. bind writes to expected language-neutral settings revision and expected target-row revision;
5. make apply idempotent with a durable owner receipt so unknown outcomes can be replayed safely;
6. emit content-free transactional change evidence or a bounded owner cursor after the owner mutation commits;
7. keep sensitivity-fenced fields structurally impossible to persist through this localized path;
8. register a Translation target only after the owner read/validate/apply/progress contract can prove those invariants.

## Forbidden shortcuts

Do not store translated values back into the base settings JSON, count rendered fallback as exact coverage, localize secret/sensitivity-fenced paths, register a provider before CAS/idempotency exists, or invent a generic settings event without a real owner transaction boundary.

## Scope of this prerequisite

This handoff records the verified boundary after #3825 and adds a source verifier. It does not add migrations, persistence, owner events, runtime fallback, Translation provider registration, or UI. It intentionally stays disjoint from the Forum UGC onboarding track and from the artifact control-plane work that is concurrently changing other `rustok-modules` files.
