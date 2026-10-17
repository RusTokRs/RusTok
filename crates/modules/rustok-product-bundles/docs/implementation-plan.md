---
id: doc://crates/modules/rustok-product-bundles/docs/implementation-plan.md
kind: module_implementation_plan
language: en
status: active
owners:
  - rustok-product-bundles
last_reviewed: 2026-10-17
---

# `rustok-product-bundles` implementation plan

## Scope

Product bundles, kits, configurable sets, item compositions, and package discounts.

## Current state

- Module schema, entities, service, port, migrations, and SQLite coverage exist.
- The persistence model couples Bundle owners and component items to tenant-scoped Product and Variant records.
- `bundle_product_id` is a zero-or-one owner association per Product; it is not a reverse component-membership lookup.
- All Bundle mutations use durable idempotency receipts at supported command ingress points.
- Dedicated UI package `rustok-product-bundles-admin` exists under the FFA/FBA architecture.
- Platform registration exists in `modules.toml` and server/admin applications.
- Bundle lifecycle outbox events are not implemented.

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`

## Local work phases

The canonical admin root is `ProductBundlesAdmin`, exported from the package
root for manifest-generated host composition. There is no alternate root alias.
The local readiness block matches the central board; host compilation alone
does not prove transport or runtime parity.

1. **Phase 1**: Domain entities (`product_bundles`, `product_bundle_translations`, `product_bundle_items`), migrations, service, receipt-bound port commands, and SQLite test suite.
2. **Phase 2**: Dedicated admin UI package `rustok-product-bundles-admin` with FFA/FBA architecture (`model.rs`, `core.rs`, `transport.rs`, `ui/leptos.rs`, bilingual locales).
3. **Phase 3**: GraphQL layer in `rustok-commerce` and regression tests.
4. **Phase 4**: Platform registration in `modules.toml`, `apps/server`, `apps/admin`, and central documentation maps.
5. **Future event-contract phase**: Define and accept Bundle lifecycle events in the shared `rustok-events` contract before adding transactional outbox publication. This includes registered event variants, schemas, canonical validation, and digest updates.

## Release and Data Rollback Readiness

- Runtime kind: Optional
- Rollback unit: Component
- Data boundary owner: `product_bundles`
- Native migrations: `m20260916_000001_create_bundles`
- Supported migration policy: ExpandContract
- Predecessor standby strategy: Hot-Standby Slot
- Rollback eligibility: AutomaticSingleAttempt
- Recovery invariants: tenant-coupled references, unique slug per tenant, at most one owned Bundle per Product, valid item positions and quantities, zero orphan records

## Invariants

- Tenant isolation: every Bundle query and mutation requires a non-nil `tenant_id`.
- Unique slug: each Bundle slug must be unique within its tenant.
- Owner cardinality: a Product can own at most one Bundle; component membership remains many-to-many in effect through item rows.
- Item integrity: quantity is at least one, position is non-negative and unique within a Bundle, and discount rate is in the inclusive range zero to one.
- Receipt integrity: a completed Bundle mutation persists its response in the same transaction as the state transition; a database unique-claim race is classified as a terminal conflict and replayed consistently.
- Event status: no Bundle lifecycle outbox event is claimed until a shared event-contract design is accepted.

## Verification

- `cargo test -p rustok-product-bundles`: Unit test suite (CRUD, ownership, slug uniqueness, translations, items management, and input validation).
- `cargo test -p rustok-product-bundles-admin --features ssr`: UI package tests.
- `cargo xtask validate-manifest`: Workspace manifest and central registry consistency.
- `cargo xtask module validate product_bundles`: Module contract validation.
