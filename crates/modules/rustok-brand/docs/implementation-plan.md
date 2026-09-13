---
id: doc://crates/modules/rustok-brand/docs/implementation-plan.md
kind: module_implementation_plan
language: en
status: active
owners:
  - rustok-brand
last_reviewed: 2026-09-15
---

# `rustok-brand` implementation plan

## Scope

Brand catalog, manufacturers, media presentation, and product associations.

## Current state

- Module structure, entity schema, migrations, service, and port created.
- In-memory SQLite test suite implemented.
- Platform registration in `modules.toml`, `docs/modules/registry.md`, and distribution crates.

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

1. **Phase 1 (Complete)**: Domain entities (`brands`, `brand_translations`, `brand_products`), migration, service, outbox events, port contracts, and SQLite test suite.
2. **Phase 2 (Complete)**: Platform registration in `modules.toml`, `apps/server`, `rustok-distribution`, and central registry.
3. **Phase 3**: GraphQL layer in `rustok-commerce` and UI integration in `rustok-product-admin`.
4. **Phase 4**: SEO target registration and storefront brand landing pages (`/brands/:slug`).

## Release and Data Rollback Readiness

- Runtime kind: Optional
- Rollback unit: Component
- Data boundary owner: `brands`
- Native migrations: `m20260915_000001_create_brands`
- Supported migration policy: ExpandContract
- Predecessor standby strategy: Hot-Standby Slot
- Rollback eligibility: AutomaticSingleAttempt
- Recovery invariants: Monotonic sequence, unique slug per tenant, zero orphan records

## Invariants

- Tenant isolation: every brand query and mutation requires a non-nil `tenant_id`.
- Unique slug: each brand slug must be unique within its tenant.
- Outbox integrity: brand modifications atomically emit outbox events.

## Verification

- `cargo test -p rustok-brand`: Unit test suite (CRUD, slug uniqueness, translation upsert, product assignment).
- `cargo test -p rustok-commerce --test graphql_surface_regression`: GraphQL regression tests.
- `cargo xtask validate-manifest`: Workspace manifest and central registry consistency.
- `cargo xtask module validate brand`: Module contract validation.

## Change rules

- Any schema modification requires an ExpandContract migration script.
- Transport additions must preserve parallel GraphQL and native server function parity.
- Admin UI changes must update both English and Russian locale resources synchronously.
