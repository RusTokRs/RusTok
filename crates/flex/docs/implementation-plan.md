# Implementation plan for `flex`

## Current state

`flex` is a capability-only custom-fields module, not a donor-persistence owner
or a separate business domain. Attached mode extends explicit donor contracts;
standalone mode owns schemas and entries. Current attached consumers are user,
product, order, and topic. Donors retain their tables and write paths.

Owner-owned contracts live in `flex::graphql`, `flex::registry`, `flex::rest`,
and `flex::standalone`. The server composes `FlexGraphqlRuntime`, SeaORM,
registry/cache adapters, and Axum REST handlers only. Localized attached
and standalone values use parallel storage; inline localized JSON is not a
canonical runtime fallback.

The field-definition cache is byte-weighted and shares ownership with a
restartable abort-on-drop invalidation consumer. Exact local events invalidate
the affected entry and listener lag clears the complete cache. The current
consumer source is process-local, so multi-replica recovery still requires a
durable event offset or shared generation owned by Flex/runtime integration.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `boundary_ready`
- Structural shape: `no_ui_boundary`
- Capability runtime is manifest-composed through `FlexModule` and
  `[provides.graphql]`; it has no module-owned UI or FBA provider port.
- `node scripts/verify/verify-flex-multilingual-contract.mjs` locks the
  multilingual storage and owner-boundary contract.

## Open results

1. **Make field-definition cache recovery durable across replicas.** Connect the
   existing exact-invalidation/full-clear consumer to a persisted event offset,
   transactional outbox consumer, or shared monotonic generation. Seed the
   consumer before serving cached definitions, invalidate before acknowledging
   the offset, and full-clear on an unverified first event, gap, or lag.
   **Depends on:** an approved inbound event consumer or generation store; the
   current process-local bus is not a cross-replica recovery source.
   **Done when:** missed-event, transport-outage, restart, and multi-replica
   evidence proves bounded convergence without relying solely on the 30-second
   TTL.

2. **Finish the owner transport extraction with targeted runtime evidence.**
   Remove remaining server Flex artifacts beyond Axum handler extraction,
   SeaORM/bootstrap adapters, and runtime composition; run targeted owner-root
   GraphQL/REST tests when compilation is available.
   **Depends on:** host-composed `FlexGraphqlRuntime` and targeted test fixtures.
   **Done when:** server holds only the allowed adapters and owner-owned roots
   execute with persistence, RBAC, errors, events, and cache invalidation.

3. **Close attached and standalone migration verification.** Verify localized
   value backfill/cleanup, PATCH merges, tenant scoping, schema validation,
   donor read/write paths, and standalone schema/entry roundtrips against the
   production persistence path.
   **Depends on:** donor migrations, standalone SeaORM adapter, and compiled
   integration test environment.
   **Done when:** no runtime reads inline localized payload as canonical, all
   live donors retain their data, and standalone integration tests are stable.

4. **Evolve advanced Flex capability only for demonstrated product needs.** Add
   future schema/entry features only with explicit donor ownership, governance,
   permissions, indexing, and documentation decisions.
   **Depends on:** a concrete product requirement and capability review.
   **Done when:** new behavior cannot be mistaken for a replacement of a
   normalized domain module or a shared donor-persistence layer.

## Verification

- `cargo xtask validate-manifest`
- `cargo xtask module validate flex`
- `node scripts/verify/verify-flex-multilingual-contract.mjs`
- Targeted owner-root GraphQL/REST, donor, migration, cache, durable
  invalidation, multi-replica recovery, and standalone schema/entry tests when
  compilation is available.

## References

- [Host cache contract inventory](../../rustok-cache/docs/host-cache-inventory.md)

## Change rules

1. Keep donor persistence and attachment tables with their owning module.
2. Keep Flex contracts and capability runtime in this crate; keep server work to
   composition, persistence adapters, and HTTP handler extraction.
3. Update the canonical Flex README, manifest, donor docs, and central module
   documentation with a capability contract change.
