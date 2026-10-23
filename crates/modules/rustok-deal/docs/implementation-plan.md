# CRM Deals & Pipelines Module Implementation Plan

## Scope
Sovereign domain capability and client surfaces for CRM Deals & Pipelines.

## Current state
- Status: `in_progress`
- Architecture: FBA / FFA
- Backend crate: `rustok-deal`
- Admin UI crate: `rustok-deal-admin`
- Storefront UI crate: `rustok-deal-storefront`

## Local work phases
- [x] Phase 1: Canonical FBA scaffolding and contract registration
- [ ] Phase 2: Domain entities and persistence migrations
- [ ] Phase 3: Application services and transactional use cases
- [ ] Phase 4: UI components, i18n and transport facade
- [ ] Phase 5: Verification and test suite lock

## Verification
- `cargo check -p rustok-deal`
- `cargo test -p rustok-deal --lib`
- `cargo xtask module validate deal`

## Change rules
- Architecture changes must update this plan and relevant ADRs.
- Keep module contract synchronized with `rustok-module.toml`.
