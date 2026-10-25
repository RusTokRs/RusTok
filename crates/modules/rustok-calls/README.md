# rustok-calls

WebRTC Calls & Conferencing capability module for RusToK

## Purpose
Provides sovereign WebRTC Calls & Conferencing domain capability for RusToK.

## Responsibility Zone
- Domain entities, state invariants and business rules for calls
- Storage and transactional boundaries for calls operations
- Emits transactional domain events via `rustok-outbox`
- Authorized via `rustok-api` permissions

## Entry points
- `CallsModule` implementing `RusToKModule`
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
