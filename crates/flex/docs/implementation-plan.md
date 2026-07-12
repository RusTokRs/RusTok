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

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `in_progress`
- Structural shape: `no_ui_boundary`
- Capability runtime is manifest-composed through `FlexModule` and
  `[provides.graphql]`; it has no module-owned UI or FBA provider port.
- `node scripts/verify/verify-flex-multilingual-contract.mjs` locks the
  multilingual storage and owner-boundary contract.

## Open results

1. **Finish the owner transport extraction with targeted runtime evidence.**
   Remove remaining server Flex artifacts beyond Axum handler extraction,
   SeaORM/bootstrap adapters, and runtime composition; run targeted owner-root
   GraphQL/REST tests when compilation is available.
   **Depends on:** host-composed `FlexGraphqlRuntime` and targeted test fixtures.
   **Done when:** server holds only the allowed adapters and owner-owned roots
   execute with persistence, RBAC, errors, events, and cache invalidation.

2. **Close attached and standalone migration verification.** Verify localized
   value backfill/cleanup, PATCH merges, tenant scoping, schema validation,
   donor read/write paths, and standalone schema/entry roundtrips against the
   production persistence path.
   **Depends on:** donor migrations, standalone SeaORM adapter, and compiled
   integration test environment.
   **Done when:** no runtime reads inline localized payload as canonical, all
   live donors retain their data, and standalone integration tests are stable.

3. **Evolve advanced Flex capability only for demonstrated product needs.** Add
   future schema/entry features only with explicit donor ownership, governance,
   permissions, indexing, and documentation decisions.
   **Depends on:** a concrete product requirement and capability review.
   **Done when:** new behavior cannot be mistaken for a replacement of a
   normalized domain module or a shared donor-persistence layer.

## Verification

- `cargo xtask validate-manifest`
- `cargo xtask module validate flex`
- `node scripts/verify/verify-flex-multilingual-contract.mjs`
- Targeted owner-root GraphQL/REST, donor, migration, cache, and standalone
  schema/entry tests when compilation is available.

## Change rules

1. Keep donor persistence and attachment tables with their owning module.
2. Keep Flex contracts and capability runtime in this crate; keep server work to
   composition, persistence adapters, and HTTP handler extraction.
3. Update the canonical Flex README, manifest, donor docs, and central module
   documentation with a capability contract change.
