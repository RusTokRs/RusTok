# Operational Finance & Cash Desks Module Architecture Contract

## Purpose
The `finance` module provides canonical Operational Finance & Cash Desks domain functionality for RusToK.

## Responsibility Zone
- Sovereign domain model and persistence for finance
- Typed events published to the transactional outbox
- RBAC permissions guard: `finance:read`, `finance:write`, `finance:admin`

## Integration
- Core runtime registration in `apps/server` via `RusToKModule`
- Event bus subscription and emission via `rustok-outbox`
- Full-text search discovery projection via `rustok-core`
- Client surfaces in `admin/` and `storefront/`

## Verification
- Unit tests: `cargo test -p rustok-finance --lib`
- Module contract verification: `cargo xtask module validate finance`

## Related Documents
- [Implementation Plan](./implementation-plan.md)
- [Crate README](../README.md)
- [CRATE_API](../CRATE_API.md)
- [Backend Implementation Guide](../../../../docs/backend/module-backend-implementation.md)

## Related Documentation
- Documentation maps and registry contracts are centralized in `docs/modules/`.
