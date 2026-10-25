# rustok-deal

CRM Deals & Pipelines capability module for RusToK

## Purpose
Provides sovereign CRM Deals & Pipelines domain capability for RusToK.

## Responsibility Zone
- Domain entities, state invariants and business rules for deal
- Storage and transactional boundaries for deal operations
- Emits transactional domain events via `rustok-outbox`
- Authorized via `rustok-api` permissions

## Entry points
- `DealModule` implementing `RusToKModule`
- Domain services in `services`
- Transactional Outbox events in `events`
- Capability integration seams in `integrations`

## Interactions
- Emits transactional domain events via `rustok-outbox`
- Search index projections via `rustok-core` search integration
- Multi-language translation targets via `rustok-translation-targets`

## Docs
- [Public Contract Reference (CRATE_API.md)](./CRATE_API.md)
- [Module Architecture Contract](./docs/README.md)
- [Implementation Plan](./docs/implementation-plan.md)
