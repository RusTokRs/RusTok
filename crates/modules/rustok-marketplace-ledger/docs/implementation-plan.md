# `rustok-marketplace-ledger` Implementation Plan

## Scope

`rustok-marketplace-ledger` owns seller ledger, escrow accounting, and transaction entries.

## Current state

Part of the Marketplace Family under `crates/rustok-commerce/docs/implementation-plan.md`.
Status: `in_progress`.

## Milestones

1. Domain core & command receipt ledger;
2. FBA port contracts & GraphQL transport;
3. Admin & storefront integration.

## Verification

- `cargo test -p rustok-marketplace-ledger`
- `cargo xtask module validate marketplace_ledger`

## Change rules

Changes must align with the unified Marketplace Family architecture in `crates/rustok-commerce/docs/implementation-plan.md`.
