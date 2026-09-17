---
id: doc://crates/modules/rustok-product-bundles/docs/implementation-plan.md
kind: module_implementation_plan
language: en
status: active
owners:
  - rustok-product-bundles
last_reviewed: 2026-09-16
---

# `rustok-product-bundles` implementation plan

## Scope

Product bundles, kits, configurable sets, item compositions, and package discounts.

## Current state

- Module schema, entities, service, port, and migrations created.
- Dedicated UI package `rustok-product-bundles-admin` created under FFA/FBA architecture.
- Platform registration in `modules.toml` and server/admin applications.

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`

## Local work phases

The canonical admin root is `ProductBundlesAdmin`, exported from the package
root for manifest-generated host composition. There is no alternate root alias.
The local readiness block matches the central board; host compilation alone
does not prove transport or runtime parity.

1. **Phase 1**: Domain entities (`product_bundles`, `product_bundle_translations`, `product_bundle_items`), migrations, service, outbox events, port contracts, and SQLite test suite.
2. **Phase 2**: Dedicated admin UI package `rustok-product-bundles-admin` with FFA/FBA architecture (`model.rs`, `core.rs`, `transport.rs`, `ui/leptos.rs`, bilingual locales).
3. **Phase 3**: GraphQL layer in `rustok-commerce` and regression tests.
4. **Phase 4**: Platform registration in `modules.toml`, `apps/server`, `apps/admin`, and central documentation maps.

## Release and Data Rollback Readiness

- Runtime kind: Optional
- Rollback unit: Component
- Data boundary owner: `product_bundles`
- Native migrations: `m20260916_000001_create_bundles`
- Supported migration policy: ExpandContract
- Predecessor standby strategy: Hot-Standby Slot
- Rollback eligibility: AutomaticSingleAttempt
- Recovery invariants: Monotonic sequence, unique slug per tenant, zero orphan records

## Invariants

- Tenant isolation: every bundle query and mutation requires a non-nil `tenant_id`.
- Unique slug: each bundle slug must be unique within its tenant.
- Item quantity: item quantity in a bundle must be >= 1.
- Outbox integrity: bundle modifications atomically emit outbox events.

## Verification

- `cargo test -p rustok-product-bundles`: Unit test suite (CRUD, slug uniqueness, translations, items management).
- `cargo test -p rustok-product-bundles-admin --features ssr`: UI package tests.
- `cargo xtask validate-manifest`: Workspace manifest and central registry consistency.
- `cargo xtask module validate product_bundles`: Module contract validation.
