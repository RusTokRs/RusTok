# rustok-tasks

Team Issues & Tasks capability module for RusToK

## Purpose
Provides sovereign Team Issues & Tasks domain capability for RusToK.

## Responsibility Zone
- Domain entities, state invariants and business rules for tasks
- Storage and transactional boundaries for tasks operations
- Emits transactional domain events via `rustok-outbox`
- Authorized via `rustok-api` permissions

## Entry points
- `TasksModule` implementing `RusToKModule`
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
