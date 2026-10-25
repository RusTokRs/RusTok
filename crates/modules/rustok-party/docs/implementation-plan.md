# Counterparty & Legal Entities Module Implementation Plan

## Scope
Sovereign domain capability and client surfaces for Counterparty & Legal Entities.

## Current state
- Status: `in_progress`
- Architecture: FBA / FFA
- Backend crate: `rustok-party`
- Admin UI crate: `rustok-party-admin`
- Storefront UI crate: `rustok-party-storefront`

## Local work phases
- [x] Phase 1: Canonical FBA scaffolding and contract registration
- [ ] Phase 2: Domain entities and persistence migrations
- [ ] Phase 3: Application services and transactional use cases
- [ ] Phase 4: UI components, i18n and transport facade
- [ ] Phase 5: Verification and test suite lock

## Verification
- `cargo check -p rustok-party`
- `cargo test -p rustok-party --lib`
- `cargo xtask module validate party`

## Change rules
- Architecture changes must update this plan and relevant ADRs.
- Keep module contract synchronized with `rustok-module.toml`.
