# Documentation `rustok-customer`

`rustok-customer` is the default storefront-customer submodule of the `ecommerce` family.

## Purpose

- schema `customers`;
- `CustomerModule` and `CustomerService`;
- module-owned admin UI package `rustok-customer/admin`;
- customer profile boundary, separated from the platform/admin user;
- optional linkage to `user_id` for `store/customers/me` scenarios;
- optional service-level bridge `customer -> user -> profile` that can return a customer together with `ProfileSummary`;
- FBA provider boundary `CustomerReadPort` for read-projection scenarios in commerce checkout and order customer snapshots.

## Scope

- the module does not depend on the `rustok-commerce` umbrella to avoid creating a cycle;
- customer profile is stored separately from the auth/user domain;
- the link to a user is optional, tenant-scoped and does not negate the autonomy of the customer model;
- the bridge to `profiles` remains an optional read-contract and does not turn the customer into a canonical public profile;
- admin UI ownership now lives in `rustok-customer/admin`; list defaults are extracted to `admin/src/core.rs`, Leptos rendering is extracted to `admin/src/ui/leptos.rs`, and CRUD calls go through `admin/src/transport/mod.rs`; native server functions use `HostRuntimeContext`; storefront GraphQL and REST transport remain in the `rustok-commerce` facade for now.

## Integration

- the module is part of the ecommerce family and must maintain its own storage/runtime boundary without returning responsibility to the umbrella `rustok-commerce`;
- storefront transport and GraphQL are still published through `rustok-commerce`, but the admin UI surface is already established as a separate module-owned surface in `rustok-customer/admin`;
- cross-module contract changes must be synchronized with `rustok-commerce` and neighboring split modules;
- `CustomerService` normalizes email before uniqueness check and storage, so create/update do not allow trimmed duplicates within a tenant; duplicate `user_id` linkage remains tenant-scoped and does not turn the customer into auth/user domain.
- `CustomerReadPort` uses the common `PortContext`/`PortError`, requires read deadline semantics and maps invalid tenant / not found to typed port errors. Its user-projection operation lets storefront consumers resolve an authenticated customer without constructing `CustomerService`; no-compile runtime smoke is captured in `contracts/evidence/customer-read-projection-runtime-smoke.json`, but `transport_verified` still requires compiled runtime execution.

## FFA split for admin

The admin package now uses framework-agnostic defaults `admin/src/core.rs`, a facade `admin/src/transport/mod.rs` over native Leptos server functions and an explicit Leptos render adapter `admin/src/ui/leptos.rs`; native server functions consume `HostRuntimeContext`; the crate root only connects the module layers and re-exports `CustomerAdmin`.

## Verification

- No-compile source/evidence gates for iterations without compilation:
  - `node scripts/verify/verify-customer-admin-boundary.mjs`
  - `node scripts/verify/verify-customer-fba-no-compile.mjs`
  - `node scripts/verify/verify-ecommerce-fba-contract-evidence.mjs`
  - `node scripts/verify/verify-ecommerce-provider-spi-evidence.mjs`
- After removing the compilation restriction:
  - `cargo xtask module validate customer`
  - `cargo xtask module test customer`
  - targeted commerce tests for the customer domain when changing runtime wiring
  - targeted customer port tests for `CustomerReadPort` deadline/error/fallback smoke before promoting FBA status

## Related documents

- [README crate](../README.md)
- [Commerce split plan](../../rustok-commerce/docs/implementation-plan.md)
