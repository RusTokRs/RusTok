---
id: doc://crates/modules/rustok-brand/docs/implementation-plan.md
kind: module_implementation_plan
language: en
status: active
owners:
  - rustok-brand
last_reviewed: 2026-11-04
---

# `rustok-brand` implementation plan

## Scope

Brand catalog, manufacturers, media presentation, and product associations.

## Current state

- Module structure, entity schema, migrations, service, and port created.
- The [file map](README.md#file-map) follows the native layout for the persisted
  backend slice. The SQLite suite applies the shipped owner migrations.
- The same localized owner and migration lifecycle scenario runs on a disposable
  PostgreSQL schema when requested explicitly.
- Platform registration in `modules.toml`, `docs/modules/registry.md`, and distribution crates.

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`

## Local work phases

1. **Phase 1 (Complete)**: Domain entities (`brands`, `brand_translations`, `brand_products`), migration, service, port contracts, and SQLite test suite.
2. **Phase 2 (Complete)**: Platform registration in `modules.toml`, `apps/server`, `rustok-distribution`, and central registry.
3. **Phase 3**: Finish and verify GraphQL/native parity and both admin clients. Commerce GraphQL and Leptos native adapters already exist. The native lint still reports admin transport/test placement and a missing Next.js package binding.
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
- Atomic localized writes: the owner saves base rows and translations in one
  transaction. The current command path has no transactional outbox publication.

## Product ownership review

Brand is outside the scaffold reference set. Its file layout and database
tests remain useful evidence for the implemented Brand tables. They do not
establish the ownership or integrity of the Product association.

- Brand owns `brand_products` today. Its assignment command checks the tenant
  and brand, but only requires a non-nil `product_id`. The Commerce GraphQL
  caller checks module availability and product update permission, then calls
  Brand without resolving the Product. The shipped table has a Brand foreign
  key and no Product foreign key. The owner test assigns a generated Product ID
  without creating a Product row.
- Confirm whether brands have an independently managed lifecycle or consumers
  outside the Product catalog. If brands are catalog-owned, evaluate moving the
  capability into Product. If Brand remains independent, evaluate a
  Product-owned assignment with an explicit Brand contract. Either design must
  reject missing and cross-tenant references and specify who handles deletion.
- Record the chosen owner and cutover in an ADR before moving data or changing
  runtime ownership. Inventory existing links, permissions, public GraphQL and
  admin consumers, migration identity, and the supported database profiles.
  Verify valid assignment, rejection and deletion through the chosen owner.

## Verification

- `cargo test -p rustok-brand --lib --test brand`: crate contract and SQLite
  scenarios against the shipped migration chain. `sqlite_migration_and_owner_behavior`
  checks fresh install, rollback/reinstall in an isolated store, rollback on a
  conflicting localized insert, tenant isolation, and explicit locale writes.
- `cargo test -p rustok-brand --test brand postgres_migration_and_owner_behavior -- --ignored`:
  the same scenario on PostgreSQL. Set `RUSTOK_BRAND_TEST_POSTGRES_URL` to a
  PostgreSQL test database with schema creation permission. The test uses and
  removes a disposable schema. Selecting this test without the URL is an error.
- `cargo test -p rustok-commerce --test graphql_surface_regression`: GraphQL regression tests.
- `cargo xtask validate-manifest`: Workspace manifest and central registry consistency.
- `cargo xtask module lint brand`: validates backend and declared UI. The
  backend slice has no reported layout violations; the admin failures listed
  above remain open independently of the Product ownership review.

On 2026-11-04, against `b163a901a6` with the Brand verification changes applied,
all four crate tests, nine SQLite integration tests, and the explicitly
selected PostgreSQL 16.13 scenario passed. The PostgreSQL run used a fresh
local instance. The Brand library and test binaries were compiled from the
current sources with `rustc` and cached Cargo dependencies while the shared
Cargo target was locked by another build. This records executed owner behavior,
not a clean Cargo or host build.

These are owner-scoped database tests. Host migration ordering and upgrades
from deployed versions, concurrent writes and error mapping, outbox delivery,
and mounted admin parity still need their own evidence. Builder checks do not
apply because Brand declares no Page Builder / Fly contribution.

## Change rules

- Any schema modification requires an ExpandContract migration script.
- Transport additions must preserve parallel GraphQL and native server function parity.
- Admin UI changes must update both English and Russian locale resources synchronously.
