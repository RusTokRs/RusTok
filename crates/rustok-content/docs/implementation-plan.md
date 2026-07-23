# Implementation plan for `rustok-content`

## Current state

`rustok-content` is the shared orchestration layer, not a product-storage
owner. Blog, forum, and pages retain their domain CRUD. This module owns shared
rich-text and locale contracts, canonical-route mapping, conversion
orchestration, audit/idempotency state, and owner-owned dashboard post
analytics. `apps/server` only composes the public roots, loaders, and owner
helper.

The accepted architecture assigns executable richtext policy to this module,
but the current implementation still lives in `rustok-core::rt_json` and is
transported as a string plus `content_json`. The target boundary is recorded in
the [central Richtext plan](../../../docs/modules/rich-text-implementation-plan.md):
neutral types move to `rustok-api::richtext`, while this module implements
profiles, strict validation, normalization, one safe HTML renderer, and one
plain-text extractor. Domain owners keep locale rows and persistence.

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

3. **Execute the atomic richtext boundary cutover.** Add the executable target
   policy under `rustok-content::richtext`, consume neutral
   `rustok-api::richtext` types, migrate every Blog/Forum/Comments and
   orchestration caller, then delete `rustok-core::rt_json`, the generic legacy
   format helper, aliases, and dual body/`content_json` paths. Keep Pages body
   on its Page Builder/Fly contract. Direct Comments writes and destination
   profile conversions must never bypass validation.
   **Depends on:** the central Richtext plan, owner-local data migrations, and
   all repository-owned transports/renderers/search projections.
   **Done when:** one strict validator, one safe HTML renderer, and one
   plain-text extractor serve all owners; no internal fallback or duplicate
   renderer remains; locale exists only in owner context/storage.

## Verification

- Contract tests cover every public use case.
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
4. Keep richtext executable behavior here and neutral wire types in
   `rustok-api`; do not create a second richtext backend owner.
