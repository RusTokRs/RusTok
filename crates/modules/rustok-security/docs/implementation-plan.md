# Security Module Implementation Plan

## Scope
Sovereign domain capability and client surfaces for Security.

## Current state
- Status: `in_progress`
- Architecture: FBA / FFA
- Backend crate: `rustok-security`
- Admin UI crate: `rustok-security-admin`
- Storefront UI crate: `rustok-security-storefront`

## Local work phases
- [x] Phase 1: Canonical FBA scaffolding and contract registration
- [ ] Phase 2: Domain entities and persistence migrations
- [ ] Phase 3: Application services and transactional use cases
- [ ] Phase 4: UI components, i18n and transport facade
- [ ] Phase 5: Verification and test suite lock

## Verification
- `cargo check -p rustok-security`
- `cargo test -p rustok-security --lib`
- `cargo xtask module validate security`

## Change rules
- Architecture changes must update this plan and relevant ADRs.
- Keep module contract synchronized with `rustok-module.toml`.
