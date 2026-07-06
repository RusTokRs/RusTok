# Implementation plan for `rustok-content`

Status: content/domain separation is complete; the module works as a shared
orchestration and rich-text/locale contract layer.

## Execution checkpoint

- Current phase: dashboard post analytics ownership in `rustok-content`
- Last checkpoint: `ContentCountSnapshot` and `load_post_stats_snapshot` moved to `rustok-content`; `apps/server::RootQuery::dashboard_stats` only composes the owner helper behind feature `mod-content` and no longer contains SQL over `nodes`/`kind = post`. The boundary is locked by `apps/server/tests/module_surface_boundary_guard.rs` without compilation.
- Next step: Close reindex drift evidence and expand conversion bridge contract coverage without returning GraphQL resolver/DTO and content analytics SQL to `apps/server`.
- Open blockers: Compile/runtime execution evidence still pending because this iteration intentionally avoided compilation.
- Hand-off notes for next agent: Maintain `npm run verify:content:orchestration` alongside any change to `ContentOrchestrationService`, `CanonicalUrlService`, collision tests, local docs or registry row.
- Last updated at (UTC): 2026-07-02T00:00:00Z

## Scope of work

- keep `rustok-content` as a shared helper/orchestration module, not a product storage owner;
- synchronize conversion semantics, canonical URL policy and local docs;
- prevent returning domain CRUD back to shared storage.

## Current state

- blog/forum/pages domain CRUD are already moved to their own modules;
- `rustok-content` owns orchestration service, audit/idempotency state and canonical URL mapping;
- canonical route GraphQL query and content GraphQL dataloaders live in `rustok-content`, while conversion mutations and DTO reside in `rustok-content-orchestration`; host only merges roots and registers owner-owned loaders;
- dashboard post analytics (`ContentCountSnapshot`, `load_post_stats_snapshot`) are already module-owned; server GraphQL does not contain SQL over `nodes`/`kind = post`;
- shared locale fallback and rich-text validation are already the canonical contract for publishable content surfaces;
- module docs and runtime boundary already reflect the post-split role.

## Stages

### 1. Contract stability

- [x] close storage split and remove product-owned transport surfaces from live runtime;
- [x] lock rich-text, locale fallback and conversion contracts;
- [x] embed RBAC/idempotency/input-safety in orchestration path;
- [x] maintain sync between orchestration contracts, event flows and module metadata through compile-free guardrail `npm run verify:content:orchestration`.

### 2. Orchestration hardening

- [x] keep canonical URL and alias semantics atomic together with outbox/reindex flows in static contract guardrail;
- [x] explicitly block canonical URL collision and alias shadowing between different targets before changing mapping/outbox state;
- [x] add targeted integration evidence for canonical URL collision and alias shadowing rollback/no-outbox scenarios;
- [ ] expand conversion coverage only through explicit bridge contracts;
- [ ] keep rich-text and locale invariants synchronized with domain modules.

### 3. Operability

- [x] evolve runbooks and observability for orchestration incidents, partial failures and reindex drift: runbook now locks verification gate `npm run verify:content:orchestration`;
- [x] cover canonical URL collision/alias shadowing guarantees with targeted integration tests (source-locked without compilation in this iteration);
- [ ] cover the following orchestration guarantees with targeted integration tests;
- [ ] document conversion policy changes simultaneously with changing runtime surface.

## Verification

- [x] compile-free guardrail covers public orchestration use-case contracts, route resolution, canonical/alias collision guards, rollback/no-outbox evidence markers and docs/registry sync: `npm run verify:content:orchestration`
- [ ] contract tests cover all public use-case orchestration and surface contracts
- `cargo xtask module validate content`
- `cargo xtask module test content`
- targeted tests for orchestration lifecycle, canonical URL policy, fallback chain and sanitize contracts

## Update rules

1. When changing content/orchestration contract, update this file first.
2. When changing public/runtime surface, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata, synchronize `rustok-module.toml`.
4. When changing shared rich-text/locale contracts, also update central docs and consumer-module references.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and currency of `README.md` and local docs.
- [x] Lock/update verification gates for current module state (`npm run verify:content:orchestration`).
