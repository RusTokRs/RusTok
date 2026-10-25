# Team Issues & Tasks Module Implementation Plan

## Scope
Sovereign domain capability and client surfaces for Team Issues & Tasks.

## Current state
- Status: `in_progress`
- Architecture: FBA / FFA
- Backend crate: `rustok-tasks`
- Admin UI crate: `rustok-tasks-admin`
- Storefront UI crate: `rustok-tasks-storefront`

## Local work phases
- [x] Phase 1: Canonical FBA scaffolding and contract registration
- [ ] Phase 2: Domain entities and persistence migrations
- [ ] Phase 3: Application services and transactional use cases
- [ ] Phase 4: UI components, i18n and transport facade
- [ ] Phase 5: Verification and test suite lock

## Verification
- `cargo check -p rustok-tasks`
- `cargo test -p rustok-tasks --lib`
- `cargo xtask module validate tasks`

## Change rules
- Architecture changes must update this plan and relevant ADRs.
- Keep module contract synchronized with `rustok-module.toml`.
