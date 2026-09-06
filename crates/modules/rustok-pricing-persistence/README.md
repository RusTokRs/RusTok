# RusToK Pricing Persistence

## Purpose

This owner-local support crate holds Pricing ORM entities and the transaction-aware bootstrap contract used while Product variants and their initial prices must commit atomically.

## Responsibilities

- Own the `prices`, `price_lists`, and `price_list_translations` SeaORM entities.
- Create, read, and delete initial variant prices through a transaction-aware API.
- Keep Product independent from the full Pricing service crate and from Commerce foundation types.

## Interactions

`rustok-pricing` re-exports these entities as its persistence model. `rustok-product` calls `BootstrapService` inside Product write transactions, and Inventory consumes pricing projections explicitly for its admin read model.

## Entry points

- `entities`
- `BootstrapService`
- `InitialPrice`

See the [Pricing module documentation](../rustok-pricing/docs/README.md).
