---
id: doc://crates/modules/rustok-product-relations/docs/implementation-plan.md
kind: module_implementation_plan
language: en
status: active
owners:
  - rustok-product-relations
last_reviewed: 2026-10-17
---

# `rustok-product-relations` implementation plan

## Scope

Product relations, merchandising associations (cross-sells, up-sells, related, accessories, alternatives), and deterministic reordering.

## Current state

- Native migrations create `product_relations`, enforce tenant-scoped Product foreign keys, and compact/enforce unique non-negative relation positions.
- `ProductRelationsPort` and `ProductRelationService` provide tenant isolation, Product locking, relation validation, and complete-order normalization.
- Receipt-bound create, update, delete, and reorder commands complete their stored response in the same transaction; plain port mutators are not external ingress.
- GraphQL surface is exposed through `rustok-commerce` (`productRelations`, `addProductRelation`, `removeProductRelation`, `reorderProductRelations`), with a required idempotency key on every mutation.
- Dedicated Leptos FFA admin package `rustok-product-relations-admin` provides native server functions and a parallel GraphQL contract.
- The SQLite fixture provisions the Product lock table required by normal CRUD paths.
- Product Relation lifecycle outbox events are not implemented.

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`

## Local work phases

The package root exports the implemented `ProductRelationsAdmin` route and
`ProductRelationsPanel` embedding surface. Native adapters use the canonical
owner `ProductRelationService`. Host compilation remains a separate gate from
the bounded SQLite tests and full native/GraphQL runtime parity.

1. **Phase 1 (Complete)**: Domain entity, migrations, service, port contracts, Product-lock-aware SQLite test suite, and durable positional invariant.
2. **Phase 2 (Complete)**: Platform registration in `modules.toml`, `apps/server`, and `docs/modules/registry.md`.
3. **Phase 3 (Complete)**: GraphQL layer in `rustok-commerce` and UI panel in `rustok-product-admin`.
4. **Phase 4**: Automated relation rules (for example category-based relations and accepted recommendation integrations).
5. **Future event-contract phase**: Define and accept Product Relation lifecycle events in the shared `rustok-events` registry before adding transactional outbox publication.

## Release and Data Rollback Readiness

- Runtime kind: Optional
- Rollback unit: Component
- Data boundary owner: `product_relations`
- Native migrations: `m20260914_000001_create_product_relations`, `m20260921_000002_enforce_product_foreign_keys`, `m20261017_000003_enforce_relation_positions`
- Supported migration policy: ExpandContract
- Predecessor standby strategy: Hot-Standby Slot
- Rollback eligibility: AutomaticSingleAttempt
- Recovery invariants: tenant-coupled Product references, no self-relations, unique pair per relation type, contiguous unique position per source/type group, zero orphan records

## Invariants

- Tenant isolation: every relation query and mutation requires a non-nil `tenant_id`.
- No self-relations: `product_id != related_product_id`.
- Pair uniqueness: a source Product, target Product, and relation type can occur only once per tenant.
- Ordering: positions are non-negative and unique per `(tenant_id, product_id, relation_type)`; create, move, delete, and reorder normalize the complete group.
- Reorder completeness: a reorder request contains every relation in the selected group exactly once.
- Receipt integrity: each completed Relation write stores its actual response in the same transaction as the state transition; terminal domain errors replay for the same key.
- Event status: no lifecycle outbox event is claimed until a shared event-contract design is accepted.

## Verification

- `cargo test -p rustok-product-relations`: Unit test suite (self-relation rejection, Product references, uniqueness, CRUD, position movement, deletion compaction, and complete reorder validation).
- `cargo test -p rustok-commerce --test graphql_surface_regression`: GraphQL regression tests.
- `cargo test -p rustok-product-relations-admin`: UI core and view-model tests.
- `cargo xtask validate-manifest`: Workspace manifest and central registry consistency.
- `cargo xtask module validate product_relations`: Contract validation.

## Change rules

- Any schema modification requires an ExpandContract migration script.
- Transport additions must preserve parallel GraphQL and native server function parity.
- Admin UI changes must update both English and Russian locale resources synchronously.
