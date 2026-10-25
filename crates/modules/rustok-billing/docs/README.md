# B2B Billing & Settlements Module Architecture Contract

## Purpose
The `billing` module provides canonical B2B Billing & Settlements domain functionality for RusToK.

## Responsibility Zone
- Sovereign domain model and persistence for billing
- Typed events published to the transactional outbox
- RBAC permissions guard: `billing:read`, `billing:write`, `billing:admin`

## Integration
- Core runtime registration in `apps/server` via `RusToKModule`
- Event bus subscription and emission via `rustok-outbox`
- Full-text search discovery projection via `rustok-core`
- Client surfaces in `admin/` and `storefront/`

## Verification
- Unit tests: `cargo test -p rustok-billing --lib`
- Module contract verification: `cargo xtask module validate billing`

## Related Documents
- [Implementation Plan](./implementation-plan.md)
- [Module Platform Overview](../overview.md)
- [Backend Implementation Guide](../../../backend/module-backend-implementation.md)

## Related Documentation
- Documentation maps and registry contracts are centralized in `docs/modules/`.
