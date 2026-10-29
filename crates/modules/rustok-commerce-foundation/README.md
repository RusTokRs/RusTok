# rustok-commerce-foundation

## Purpose

`rustok-commerce-foundation` provides shared DTOs, entities, search helpers, and errors for split commerce modules.

## Responsibilities

- Shared commerce DTOs.
- Shared SeaORM entities.
- Shared commerce error surface.
- Shared query/search helpers.

## Interactions

- Used by `rustok-product`, `rustok-pricing`, `rustok-inventory`, and `rustok-commerce`.

## Entry points

- `dto::*`
- `entities::*`
- `CommerceError`
- `CommerceResult`

## Documentation

- See [CRATE_API.md](CRATE_API.md) for full contract specification.
- See also `docs/README.md`.
