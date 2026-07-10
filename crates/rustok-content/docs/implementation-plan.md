# Implementation plan for `rustok-content`

## Current state

`rustok-content` is the shared orchestration layer, not a product-storage
owner. Blog, forum, and pages retain their domain CRUD. This module owns shared
rich-text and locale contracts, canonical-route mapping, conversion
orchestration, audit/idempotency state, and owner-owned dashboard post
analytics. `apps/server` only composes the public roots, loaders, and owner
helper.

The canonical URL guard already rejects cross-target canonical/alias conflicts
before state or outbox mutations; aliases are resolved before canonical routes.
The local README and runbook record the stable contract and incident recovery
procedure.

## Open results

1. **Prove reindex-drift recovery end to end.** Add targeted integration
   evidence for a corrected canonical route and the affected domain reindex,
   including partial-locale failure handling. Do not reintroduce shared-domain
   CRUD or a server-owned content SQL path.
   **Depends on:** typed reindex entry points in `rustok-index` and the owning
   domain module.
   **Done when:** the recovery path is covered by a targeted integration test
   and the runbook remains executable.

2. **Finish public conversion-bridge contract coverage.** Cover the remaining
   promote, demote, split, and merge outcomes through explicit bridge contracts,
   keeping RBAC, idempotency, canonical mutations, audit records, and outbox
   publication in one transaction.
   **Depends on:** `rustok-content-orchestration` bridge and the owning blog/
   forum contracts.
   **Done when:** targeted tests cover every public outcome and failure has no
   persisted orchestration state or outbox side effect.

3. **Keep shared rich-text and locale invariants aligned with consumers.** When
   a domain changes conversion semantics or a public content contract, update
   the shared contract, local README/runbook, module metadata, and consumer
   references in the same change.
   **Depends on:** the change-owning domain module.
   **Done when:** no consumer relies on a divergent fallback, validation, or
   route-resolution rule.

## Verification

- `npm run verify:content:orchestration` — fast guardrail for public
  orchestration contracts, route resolution, RBAC/idempotency/audit/outbox,
  canonical/alias collision rollback evidence, and documentation/registry sync.
- `cargo xtask module validate content`
- `cargo xtask module test content`
- Targeted tests for orchestration lifecycle, canonical-route recovery, locale
  fallback, and rich-text sanitization.

## Change rules

1. Keep `rustok-content` a shared contract/orchestration module; domain CRUD
   remains with its owner.
2. Update [the local README](./README.md) and [runbook](./runbook.md) with a
   public or operational contract change.
3. Update `rustok-module.toml` and consumer references when module metadata or
   shared rich-text/locale contracts change.
