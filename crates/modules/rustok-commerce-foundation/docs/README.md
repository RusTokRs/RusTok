# Documentation `rustok-commerce-foundation`

`rustok-commerce-foundation` is a shared support crate for the split commerce family.
It holds common DTOs, entities, errors and search/query helpers, without becoming
an independent domain module.

## Purpose

- publish a common foundation surface for split commerce crates;
- keep shared DTOs, entities and error contracts outside the umbrella module;
- reduce duplication between `product`, `pricing`, `inventory` and other commerce crates.

## Responsibilities

- shared commerce DTOs;
- shared SeaORM entities;
- unified `CommerceError` / `CommerceResult`;
- shared query/search helpers for the commerce family;
- no independent transport/runtime orchestration layer.

## Integration

- used by `rustok-product`, `rustok-pricing`, `rustok-inventory` and `rustok-commerce`;
- must remain a dependency-only support crate without its own domain/business boundary;
- changes to shared DTOs/entities must be synchronized with consumer crates and umbrella docs;
- must not absorb logic that already belongs to a stable bounded context.

## Verification

- structural verification: shared docs and consumer expectations must remain synchronized;
- targeted compile/tests are executed when changing shared DTOs/entities/error surface;
- any incompatible changes require synchronization of consumer crates.

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Umbrella `commerce` plan](../../rustok-commerce/docs/implementation-plan.md)
