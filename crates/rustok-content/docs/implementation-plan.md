# Implementation plan for `rustok-content`

## Current state

`rustok-content` is the shared orchestration layer, not a product-storage
owner. Blog, forum, and pages retain their domain CRUD. This module owns shared
rich-text and locale contracts, canonical-route mapping, conversion
orchestration, audit/idempotency state, and owner-owned dashboard post
analytics. `apps/server` only composes the public roots, loaders, and owner
helper.

The accepted architecture assigns executable richtext policy to this module,
and the first target policy is now implemented in `src/richtext/`: the
`article`, `discussion`, and `comment` profiles validate the full initial
tree grammar, reject unknown structure/attributes, normalize Tiptap default
attributes and mark order, enforce raw/typed size limits, render escaped
semantic HTML, and extract plain text. The target boundary is recorded in
the [central Richtext plan](../../../docs/modules/rich-text-implementation-plan.md):
neutral types live in `rustok-api::richtext`, while this module implements the
executable policy. Domain owners keep locale rows and persistence. Existing
owner transports have not yet been switched, so `rustok-core::rt_json`, the
generic helper, and `content_json` remain cutover work rather than supported
new inputs.

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

3. **Execute the atomic richtext boundary cutover.** Consume the implemented
   executable target policy under `rustok-content::richtext` and neutral
   `rustok-api::richtext` types, migrate every Blog/Forum/Comments and
   orchestration caller, then delete `rustok-core::rt_json`, the generic legacy
   format helper, aliases, and dual body/`content_json` paths. Keep Pages body
   on its Page Builder/Fly contract. Direct Comments writes and destination
   profile conversions must never bypass validation.
   **Depends on:** the central Richtext plan, owner-local data migrations, and
   all repository-owned transports/renderers/search projections.
   **Current evidence:** `cargo test -p rustok-content richtext` covers the
   accepted article fixture, profile manifest, tree grammar, links, escaping,
   normalization, size limits, and projections. Comments storage/service/port
   and the Blog comment consumer now use the typed contract; their migration
   rejects non-canonical legacy rows before dropping the selector. Blog posts
   and Forum remain pending, and orchestration fails closed when their
   comments/replies would cross the mixed-contract boundary.
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
