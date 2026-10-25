# CRM Deals & Pipelines Module Architecture Contract

## Purpose
The `deal` module provides canonical CRM Deals & Pipelines domain functionality for RusToK.

## Responsibility Zone
- Sovereign domain model and persistence for deal
- Typed events published to the transactional outbox
- RBAC permissions guard: `deal:read`, `deal:write`, `deal:admin`

## Integration
- Core runtime registration in `apps/server` via `RusToKModule`
- Event bus subscription and emission via `rustok-outbox`
- Full-text search discovery projection via `rustok-core`
- Client surfaces in `admin/` and `storefront/`

## Verification
- Unit tests: `cargo test -p rustok-deal --lib`
- Module contract verification: `cargo xtask module validate deal`

## Related Documents
- [Implementation Plan](./implementation-plan.md)
- [Module Platform Overview](../overview.md)
- [Backend Implementation Guide](../../../backend/module-backend-implementation.md)

## Related Documentation
- Documentation maps and registry contracts are centralized in `docs/modules/`.
