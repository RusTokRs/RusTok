# WebRTC Calls & Conferencing Module Architecture Contract

## Purpose
The `calls` module provides canonical WebRTC Calls & Conferencing domain functionality for RusToK.

## Responsibility Zone
- Sovereign domain model and persistence for calls
- Typed events published to the transactional outbox
- RBAC permissions guard: `calls:read`, `calls:write`, `calls:admin`

## Integration
- Core runtime registration in `apps/server` via `RusToKModule`
- Event bus subscription and emission via `rustok-outbox`
- Full-text search discovery projection via `rustok-core`
- Client surfaces in `admin/` and `storefront/`

## Verification
- Unit tests: `cargo test -p rustok-calls --lib`
- Module contract verification: `cargo xtask module validate calls`

## Related Documents
- [Implementation Plan](./implementation-plan.md)
- [Module Platform Overview](../overview.md)
- [Backend Implementation Guide](../../../backend/module-backend-implementation.md)

## Related Documentation
- Documentation maps and registry contracts are centralized in `docs/modules/`.
