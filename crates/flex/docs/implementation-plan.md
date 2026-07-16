# Implementation plan for `flex`

## Current state

`flex` is a capability-only custom-fields module, not a donor-persistence owner or a separate
business domain. Attached mode extends explicit donor contracts; standalone mode owns schemas and
entries. Current attached consumers are user, product, order and topic. Donors retain their tables
and write paths.

Owner-owned contracts live in `flex::graphql`, `flex::registry`, `flex::rest` and
`flex::standalone`. The server composes `FlexGraphqlRuntime`, SeaORM, registry/cache adapters and
Axum REST handlers only. Localized attached and standalone values use parallel storage; inline
localized JSON is not a canonical runtime fallback.

The field-definition cache is byte-weighted and keeps the local EventBus consumer as a low-latency
exact-invalidation path. Durable convergence is source-complete:

- `flex_field_definition_cache_generation` is a singleton database generation;
- transaction-local database triggers advance it for every INSERT/UPDATE/DELETE on
  `user_field_definitions`, `product_field_definitions`, `order_field_definitions` and
  `topic_field_definitions`, including reorder and soft-delete updates;
- migrations are ordered `000001` through `000004`, so the shared generation exists before owner
  triggers and reverse rollback removes triggers before the singleton table/function;
- every serving runtime reads the durable generation, clears the complete cache before marking the
  generation applied, polls every five seconds and repeats the clear on advancement;
- database failure or generation regression is fail-closed, the supervised worker restarts, and its
  handle is a critical runtime guardrail/readiness dependency;
- the process-local consumer remains restartable/abort-on-drop and full-clears on local lag.

Source completion is not compiled or multi-replica verified until the targeted migration/runtime
suite passes on one revision.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `boundary_ready`
- Structural shape: `no_ui_boundary`
- Capability runtime is manifest-composed through `FlexModule` and `[provides.graphql]`; it has no
  module-owned UI or FBA provider port.
- `node scripts/verify/verify-flex-multilingual-contract.mjs` locks the multilingual storage and
  owner-boundary contract.

## Open results

1. **Execute durable field-cache recovery evidence.** Verify PostgreSQL and SQLite generation
   triggers for all four donor tables, seed-before-clear startup, generation advancement,
   concurrent mutations, database outage/recovery, regression handling, worker restart/readiness
   and multi-replica convergence without relying on the 30-second TTL.
   **Depends on:** a compiled migration/server environment with at least two serving replicas.
   **Done when:** every mutation class advances the singleton generation transactionally and all
   replicas clear before recording the new applied generation.

2. **Finish the owner transport extraction with targeted runtime evidence.** Remove remaining
   server Flex artifacts beyond Axum handler extraction, SeaORM/bootstrap adapters and runtime
   composition; run targeted owner-root GraphQL/REST tests when compilation is available.
   **Depends on:** host-composed `FlexGraphqlRuntime` and targeted test fixtures.
   **Done when:** server holds only the allowed adapters and owner-owned roots execute with
   persistence, RBAC, errors, events and cache invalidation.

3. **Close attached and standalone migration verification.** Verify localized value
   backfill/cleanup, PATCH merges, tenant scoping, schema validation, donor read/write paths and
   standalone schema/entry roundtrips against production persistence.
   **Depends on:** donor migrations, standalone SeaORM adapter and compiled integration fixtures.
   **Done when:** no runtime reads inline localized payload as canonical, all live donors retain
   their data and standalone integration tests are stable.

4. **Evolve advanced Flex capability only for demonstrated product needs.** Add future
   schema/entry features only with explicit donor ownership, governance, permissions, indexing and
   documentation decisions.
   **Depends on:** a concrete product requirement and capability review.
   **Done when:** new behavior cannot be mistaken for a replacement of a normalized domain module
   or a shared donor-persistence layer.

## Verification

- `cargo xtask validate-manifest`
- `cargo xtask module validate flex`
- `node scripts/verify/verify-flex-multilingual-contract.mjs`
- `cargo check -p flex --lib`
- `cargo check -p rustok-auth --lib`
- `cargo check -p rustok-product --lib`
- `cargo check -p rustok-commerce --lib`
- `cargo check -p rustok-forum --lib`
- `cargo check -p rustok-server --lib`
- `cargo test -p rustok-server --test field_definition_cache_generation_guard`
- Targeted PostgreSQL/SQLite migration, cache generation, database recovery, readiness and
  multi-replica tests.

## References

- [Host cache contract inventory](../../rustok-cache/docs/host-cache-inventory.md)
- [Cache capability implementation plan](../../rustok-cache/docs/implementation-plan.md)

## Change rules

1. Keep donor persistence and attachment tables with their owning module.
2. Keep reusable generation/trigger helpers and Flex contracts in this crate; owner migrations
   install triggers on their own tables.
3. Keep server work to composition, persistence adapters, reconciliation and HTTP handler
   extraction.
4. Update the canonical Flex README, manifest, donor docs and central module documentation with a
   capability contract change.
