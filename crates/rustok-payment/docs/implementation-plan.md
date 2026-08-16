# `rustok-payment` implementation planning

## Scope

`rustok-payment` owns collection lifecycle, provider registry, provider operation journals, checkout compensation, and payment execution.

## Current state

The ecommerce-family implementation plan is maintained in `crates/rustok-commerce/docs/implementation-plan.md#payment-workstream`.
This module is currently `boundary_ready` with FBA registry `payment-fba-registry.json`.

## Milestones

1. Provider integration & webhook intake;
2. Checkout compensation & payment execution ports;
3. Capture, refund, and reconciliation execution.

## Verification

- `cargo test -p rustok-payment`
- `cargo xtask module validate payment`

## Change rules

All payment architectural changes must be coordinated with the ecommerce family plan in `crates/rustok-commerce/docs/implementation-plan.md`.
