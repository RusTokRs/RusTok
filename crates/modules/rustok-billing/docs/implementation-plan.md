# B2B Billing & Settlements Module Implementation Plan

## Scope
Sovereign domain capability and client surfaces for B2B Billing & Settlements.

## Current state
- Status: `in_progress`
- Architecture: FBA / FFA
- Backend crate: `rustok-billing`
- Admin UI crate: `rustok-billing-admin`
- Storefront UI crate: `rustok-billing-storefront`

## Local work phases
- [x] Phase 1: Canonical FBA scaffolding and contract registration
- [ ] Phase 2: Domain entities and persistence migrations
- [ ] Phase 3: Application services and transactional use cases
- [ ] Phase 4: UI components, i18n and transport facade
- [ ] Phase 5: Verification and test suite lock

## Verification
- `cargo check -p rustok-billing`
- `cargo test -p rustok-billing --lib`
- `cargo xtask module validate billing`

## Change rules
- Architecture changes must update this plan and relevant ADRs.
- Keep module contract synchronized with `rustok-module.toml`.
