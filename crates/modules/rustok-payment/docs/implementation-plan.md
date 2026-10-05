# `rustok-payment` implementation planning

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`

## Scope

`rustok-payment` owns collection lifecycle, provider registry, provider operation journals, checkout compensation, and payment execution.

Provider-operation journals are tenant-scoped at every read and lifecycle mutation.
Provider operation requests/results also pass bounded metadata/payload and reference validation at the provider SPI before external execution and durable journal persistence. Journal creation validates that the payment collection belongs to the requested tenant and that an optional refund belongs to the same tenant and collection; the database migration `m20261005_000121_enforce_provider_operation_ownership` rejects cross-tenant or cross-collection provider-operation inserts.

## Current state

The ecommerce-family implementation plan is maintained in `crates/modules/rustok-commerce/docs/implementation-plan.md#payment-workstream`.
This module is currently `boundary_ready` with FBA registry `payment-fba-registry.json`.

## Milestones

1. Provider integration & webhook intake;
2. Checkout compensation & payment execution ports;
3. Capture, refund, and reconciliation execution.

## Verification

- `cargo test -p rustok-payment`
- `cargo xtask module validate payment`

## Storefront transport fallback policy

The payment storefront transport selects native server functions or GraphQL through `execute_selected_transport`. The owner facade operations `create_payment_collection`, `fetch_payment_collection`, and `fetch_refund_summary` route through their respective adapters without broad fallback.

## Change rules

All payment architectural changes must be coordinated with the ecommerce family plan in `crates/modules/rustok-commerce/docs/implementation-plan.md`.
