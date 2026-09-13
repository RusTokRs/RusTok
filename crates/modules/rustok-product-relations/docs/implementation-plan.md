---
id: doc://crates/modules/rustok-product-relations/docs/implementation-plan.md
kind: module_implementation_plan
language: en
status: active
owners:
  - rustok-product-relations
last_reviewed: 2026-09-14
---

# `rustok-product-relations` implementation plan

## Scope

Product relations, merchandising associations (cross-sells, up-sells, related, accessories, alternatives) and reordering.

## Current state

- Native migration `m20260914_000001_create_product_relations` implemented and verified.
- `ProductRelationsPort` and `ProductRelationService` implemented with tenant isolation and outbox events.
- GraphQL surface exposed through `rustok-commerce` (`productRelations`, `addProductRelation`, `removeProductRelation`, `reorderProductRelations`).
- Admin UI panel `ProductRelationsPanel` integrated in `rustok-product-admin`.
- In-memory SQLite tests pass 4/4; GraphQL surface regression tests pass; Product Admin tests pass 52/52.

## FFA/FBA Status

```yaml
ffa:
  status: not_started
  shape: none
fba:
  status: boundary_ready
  shape: no_ui_boundary
```

## Local work phases

1. **Phase 1 (Complete)**: Domain entity, migration, service, outbox events, port contracts, and SQLite test suite.
2. **Phase 2 (Complete)**: Platform registration in `modules.toml`, `apps/server`, and `docs/modules/registry.md`.
3. **Phase 3 (Complete)**: GraphQL layer in `rustok-commerce` and UI panel in `rustok-product-admin`.
4. **Phase 4**: Automated relation rules (e.g. category-based related products, AI-driven recommendations via `rustok-ai`).

## Release and Data Rollback Readiness

- Runtime kind: Optional
- Rollback unit: Component
- Data boundary owner: `product_relations`
- Native migrations: `m20260914_000001_create_product_relations`
- Supported migration policy: ExpandContract
- Predecessor standby strategy: Hot-Standby Slot
- Rollback eligibility: AutomaticSingleAttempt
- Recovery invariants: Monotonic sequence, unique pair per relation type, zero orphan records

## Invariants

- Tenant isolation: every relation query and mutation requires a non-nil `tenant_id`.
- No self-relations: `product_id != related_product_id`.
- Idempotency: creating an already existing relation pair of the same type is rejected with `RelationAlreadyExists`.
- Outbox integrity: relation modifications atomically emit outbox events.

## Verification

- `cargo test -p rustok-product-relations`: Unit test suite (self-relation rejection, uniqueness, CRUD, ordering).
- `cargo test -p rustok-commerce --test graphql_surface_regression`: GraphQL regression tests.
- `cargo test -p rustok-product-admin`: UI core and view-model tests.
- `cargo xtask validate-manifest`: Workspace manifest and central registry consistency.
- `cargo xtask module validate product_relations`: Contract validation.

## Change rules

- Any schema modification requires an ExpandContract migration script.
- Transport additions must preserve parallel GraphQL and native server function parity.
- Admin UI changes must update both English and Russian locale resources synchronously.
