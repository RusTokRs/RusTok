# rustok-customer

## Purpose

`rustok-customer` is the default storefront customer submodule of the `Ecommerce` family.

## Responsibilities

- Own the storefront customer profile schema and service logic.
- Keep customer identity separate from admin/runtime users while allowing tenant-scoped optional linkage by `user_id`.
- Expose an optional service-level `customer -> user -> profile` bridge without collapsing the two domains.
- Prepare a stable customer boundary for later checkout and payment flows.
- Publish `CustomerReadPort` as the transport-neutral read-projection provider for commerce checkout and order customer snapshots.
- Publish a module-owned Leptos admin UI package in `admin/` for tenant-scoped customer operations.

## Interactions

- Depends on `rustok-core` for module contracts and customer permission vocabulary.
- Depends on `rustok-profiles` only for optional bridge/read enrichment contracts.
- Used by `rustok-commerce` as the default customer submodule of the ecommerce family.
- Provides in-process FBA read-projection operations (`read_customer_projection`, `read_customer_projection_by_user`, `list_customer_projections`) with shared `PortContext`/`PortError` semantics; authored runtime smoke tests cover deadline enforcement, typed errors, and tenant-scoped fallback listing while boundary promotion waits for compiled execution.
- Normalizes email before uniqueness checks and persistence, so trimmed duplicate create/update requests are rejected within a tenant while cross-tenant customer identities remain isolated.
- Keeps an optional `user_id` link to the platform user record without collapsing customer and user into one domain model.
- `apps/admin` consumes `rustok-customer-admin` through manifest-driven composition; native admin server functions consume `HostRuntimeContext`, while storefront GraphQL/REST customer transport remains in `rustok-commerce`.

## Entry points

- `CustomerModule`
- `CustomerService`
- `CustomerReadPort`
- `rustok-customer-admin`
- `dto::*`
- `entities::*`

## Verification

No-compile customer boundary checks for source/evidence-only iterations:

- `node scripts/verify/verify-customer-fba-no-compile.mjs`
- `node scripts/verify/verify-ecommerce-fba-contract-evidence.mjs`
- `node scripts/verify/verify-ecommerce-provider-spi-evidence.mjs`

Compiled module gates (`cargo xtask module validate customer`, `cargo xtask module test customer`, and targeted `cargo test -p rustok-customer ...`) remain required before promoting FBA beyond `in_progress`, but are intentionally not part of no-compile iterations.

See also `docs/README.md`.
