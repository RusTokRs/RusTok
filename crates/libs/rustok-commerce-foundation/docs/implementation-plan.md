# rustok-commerce-foundation implementation plan

## Current state

`rustok-commerce-foundation` is the dependency-only shared contract layer for
the split commerce family. It contains common DTOs, SeaORM entities, errors,
and the product-translation search helper used by product, pricing, inventory,
cart, region, and the commerce umbrella. It owns no transport, runtime,
service orchestration, or UI boundary.

## Readiness

- FFA/FBA status: `not_started` — the crate has no UI or transport surface.
- Owner: commerce platform.
- Boundary: a type belongs here only when two or more stable commerce owners
  need the same contract. Domain services, workflow policy, and host adapters
  remain with their bounded-context owner.
- Existing guard: `product_translation_title_search_condition` remains outside
  `apps/server`, protected by
  `product_translation_search_helper_is_not_server_owned` in
  `apps/server/tests/module_surface_boundary_guard.rs`.

## Next results

1. **Define consumer acceptance for public contract changes.** Maintain the
   affected-consumer matrix for DTO, entity, error, and search-helper changes;
   run the targeted compile/test set before merging incompatible updates. Done
   when a foundation change identifies its consumers and verifies their public
   use of the modified contract.
2. **Reconcile ownership before adding shared surface.** Move a type here only
   after confirming genuine multi-owner reuse; return domain-specific services,
   policy, and persistence orchestration to product, pricing, inventory,
   region, or the commerce umbrella. Done when new foundation additions have a
   named consumer set and no domain execution logic.
3. **Keep the search-helper boundary executable.** Extend the module-surface
   guard whenever shared query helpers are added or relocated. Done when no
   commerce query helper has a duplicate server-owned implementation.

## Verification

- `cargo check -p rustok-commerce-foundation`
- `cargo test -p rustok-server product_translation_search_helper_is_not_server_owned`
- Targeted consumer checks for the crates affected by a changed public type.

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [Commerce umbrella plan](../../rustok-commerce/docs/implementation-plan.md)
