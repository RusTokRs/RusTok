# rustok-distribution

## Purpose

`rustok-distribution` assembles the module registry selected by a RusToK
distribution build.

## Responsibilities

- Own compile-time module selection and `ModuleRegistry` composition.
- Own explicit compile-time cross-module adapter selection. The
  `ai-translation` feature requires both owner modules and publishes only the
  Translation-owned lazy machine-translation factory through neutral runtime
  extensions. The production server profile selects this bridge; when the
  deployment result keyring is absent, the factory resolves to no machine
  provider and leaves manual Translation workflows available.
- Generate the deterministic Cargo dependency fragment, promoted-module
  registry source, and machine-readable composition manifest consumed only in
  immutable static-distribution CI workspaces.
- Provide the same selected registry to HTTP hosts and standalone operations.
- Keep routing, lifecycle, command providers and domain logic outside this crate.
- Keep executable hosts capability-neutral: they call
  `build_runtime_extensions(...)` and never import selected bridge types.
- Compose the native Product and Channel Index projections published by their
  owners. Distribution owns the adapter registration and maps published rows
  into Index records. The owner SQL is composed in the same PostgreSQL
  statement or existing repeatable-read transaction.
- Bind ProductVariant replay through the Product owner port. The Index schema
  and mutation mapping are shared by the embedded and gRPC deployments.
- Compose the native Taxonomy adapters for Product and Groups. The adapters keep
  transaction-bound calls on the consumer's transaction while Taxonomy owns the
  queries and mutations.

## Interactions

- `apps/server` uses the registry for HTTP host composition.
- Standalone operational adapters can use the same registry without importing
  `apps/server`.
- `rustok-installer` receives the trusted `composition_identity()` through its
  CLI and HTTP hosts; installer plans never accept this identity from a wizard.
- Module CLI adapters remain owner-local and are aggregated separately by
  `rustok-cli-registry`.
- `rustok-modules` supplies an owner-validated, digest-pinned distribution work
  item to `generate_static_distribution`; the generator performs no source
  fetching, compilation, publication, or runtime mutation.

## Documentation

- See [CRATE_API.md](CRATE_API.md) for full contract specification.

## ProductVariant replay verification

Build `rustok-product-catalog-service` with Cargo's `service` feature. Set
`RUSTOK_PRODUCT_VARIANT_REPLAY_TEST_BINARY` to the binary path and
`RUSTOK_MIGRATION_SMOKE_ADMIN_URL` to a disposable PostgreSQL connection whose
role can create databases.

```sh
cargo test -p rustok-distribution --features mod-product --lib standalone_product_variant_replay_materializes_in_a_separate_index_database -- --ignored
```

The fixture starts the standalone Product process and creates two databases.
It checks replay pagination, tenant scoping, credential grants, mutation identity
parity with the embedded port, the minimum revision fence, and the persisted Index
record through creation, update, and deletion.

## Selected Inventory Index bridge

When the actual Inventory module is selected, Distribution contributes its native
`inventory-stock-primary` / `rustok-inventory/stock` schema/source and real sealed event listener
through existing catalogs. Auth supplies current authenticated service read contexts; Inventory
owns source history; Index owns all derived persistence/job/checkpoint effects. Neither host nor
Distribution queries Inventory tables. See the [actual contract/configuration](docs/inventory-index.md).

## Known Limitations / Pending Implementation

Both Product replay sources select the owner port for an independent Product
database. Absence and query admission still require the embedded snapshot.
The mixed Product refresh stream and storefront queries still need separate
end-to-end verification. See the [owner read contract ADR](../../../DECISIONS/2026-11-08-distribution-index-owner-read-contracts.md).

The Inventory bridge remains uncompiled and unrun. Current page failures park terminally. Automatic
failed-job backoff/original-job operator recovery is unwired. Real service credentials/current
policy, backend/concurrency/broker/restart/deployment and full cross-client recovery remain open
in the existing [Inventory](../rustok-inventory/docs/implementation-plan.md) and
[Index](../rustok-index/docs/implementation-plan.md) plans. Source selection and durable initial
enqueue are not runtime verification, readiness or Product step-8 completion. No stock transport/
UI surface or missing historical publication is fabricated by this bridge.
