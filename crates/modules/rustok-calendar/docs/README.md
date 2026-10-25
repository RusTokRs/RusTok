# Calendar & Appointment Scheduling Module Architecture Contract

## Purpose
The `calendar` module provides canonical Calendar & Appointment Scheduling domain functionality for RusToK.

## Responsibility Zone
- Sovereign domain model and persistence for calendar
- Typed events published to the transactional outbox
- RBAC permissions guard: `calendar:read`, `calendar:write`, `calendar:admin`

## Integration
- Core runtime registration in `apps/server` via `RusToKModule`
- Event bus subscription and emission via `rustok-outbox`
- Full-text search discovery projection via `rustok-core`
- Client surfaces in `admin/` and `storefront/`

## Verification
- Unit tests: `cargo test -p rustok-calendar --lib`
- Module contract verification: `cargo xtask module validate calendar`

## Related Documents
- [Implementation Plan](./implementation-plan.md)
- [Module Platform Overview](../overview.md)
- [Backend Implementation Guide](../../../backend/module-backend-implementation.md)

## Related Documentation
- Documentation maps and registry contracts are centralized in `docs/modules/`.
