# `rustok-marketplace-commission` Implementation Plan

## Scope

`rustok-marketplace-commission` owns commission calculation, rate rules, and fee assignment.

## Current state

Part of the Marketplace Family under `crates/rustok-commerce/docs/implementation-plan.md`.
Status: `in_progress`.

## Milestones

1. Domain core & command receipt ledger;
2. FBA port contracts & GraphQL transport;
3. Admin & storefront integration.

## Verification

- `cargo test -p rustok-marketplace-commission`
- `cargo xtask module validate marketplace_commission`

## Change rules

Changes must align with the unified Marketplace Family architecture in `crates/rustok-commerce/docs/implementation-plan.md`.
