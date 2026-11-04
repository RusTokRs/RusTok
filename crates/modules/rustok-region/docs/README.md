# `rustok-region` Documentation

`rustok-region` — default region submodule of the `ecommerce` family.

## Purpose

- `regions` schema;
- `RegionModule` and `RegionService`;
- region boundary for country/currency/tax baseline;
- typed `tax_provider_id` as a region-owned baseline hook for tax provider selection;
- optional channel-scoped override map in `metadata.channel_tax_provider_ids` (string or object `{provider_id|provider}`) used by the cart/tax runtime only when `channel_id` is present;
- module-owned admin UI for region CRUD;
- module-owned storefront UI for public region discovery;
- default region lookup by `region_id` or country.

## Scope

- the module owns the `regions` table and baseline policy for countries, currency and tax flags;
- the module does not own tenant locales: they remain platform-core data;
- the channel-specific tax-provider override map remains a compatibility metadata contract and does not replace the typed baseline `tax_provider_id`;
- locale/currency orchestration over the baseline still lives in the umbrella `rustok-commerce`, which links `regions` to tenant locale policy;
- operator-facing admin CRUD is now published by the module itself via `rustok-region/admin`, not through the aggregate `commerce`.
- public storefront read-side is now also published by the module itself via `rustok-region/storefront`, not through the aggregate storefront route.

## Integration

- the module is part of the ecommerce family and must maintain its own storage/runtime boundary without returning responsibility to the umbrella `rustok-commerce`;
- storefront transport for region discovery is still published through `rustok-commerce`;
- the storefront route `/modules/regions` is now published by the module itself via `[provides.storefront_ui]`, keeping GraphQL transport as a parallel fallback contract;
- admin UI is connected by the host application `apps/admin` via manifest-driven `[provides.admin_ui]`;
- Leptos admin/storefront packages use native `#[server]` functions as the default internal data layer and read the effective locale from `UiRouteContext.locale`; admin native transport consumes `rustok_api::HostRuntimeContext` for DB access and does not depend on a host-wide `AppContext`; storefront route/tax/country summary formatting, selected-region resolution and error status/view-model mapping are moved to framework-agnostic `storefront/src/core.rs`, and native/GraphQL transport paths are separated through `storefront/src/transport/` with a typed fallback error envelope; Leptos render code lives in explicit adapter files `admin/src/ui/leptos.rs` and `storefront/src/ui/leptos.rs`.

## Region command and query file map

| File | Responsibility |
| --- | --- |
| `src/domain/region.rs` | Normalize country, currency, and tax provider values without database access |
| `src/dto/region.rs` | Define Region inputs, translations, and projections |
| `src/services/region/mod.rs` | Construct `RegionService` and wire commands and queries |
| `src/services/region/commands.rs` | Validate writes, own the transaction, and assemble the response before commit |
| `src/services/region/queries.rs` | Serve tenant-scoped reads and country lookup |
| `src/services/region/repository.rs` | Load and map owner rows, apply read-side locale fallback, and write related rows through the supplied transaction |
| `src/migrations/mod.rs` | Provide the owner migration chain used in runtime and database tests |
| `src/tests/contract.rs`, `tests/region_service_test.rs` | Verify module wiring and owner behavior on the migrated schema |

Commands assemble the response inside the same transaction as the base row,
translation rows, country policy, and translation change journal. Return the
response after commit succeeds. Read callers outside the module should use
`RegionReadPort` for the tenant-scoped projection.

## Verification

- `cargo xtask module validate region`
- `cargo xtask module test region`
- `cargo test -p rustok-region --lib --test region_service_test --test module`
- See the owner plan for the explicit PostgreSQL scenario and the remaining reference gates.
- `node scripts/verify/verify-region-admin-boundary.mjs`
- `cargo check -p rustok-region-admin --lib`
- `cargo check -p rustok-region-storefront --lib`
- targeted commerce tests for storefront region transport when changing runtime wiring

## Related documents

- [README crate](../README.md)
- [`rustok-region` implementation plan](./implementation-plan.md)
- [`commerce` umbrella plan](../../rustok-commerce/docs/implementation-plan.md)
