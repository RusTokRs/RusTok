# Counterparty & Legal Entities Module Architecture Contract

## Purpose
The `party` module provides canonical Counterparty & Legal Entities domain functionality for RusToK.

## Responsibility Zone
- Sovereign domain model and persistence for party
- Typed events published to the transactional outbox
- RBAC permissions guard: `party:read`, `party:write`, `party:admin`

## Integration
- Core runtime registration in `apps/server` via `RusToKModule`
- Event bus subscription and emission via `rustok-outbox`
- Full-text search discovery projection via `rustok-core`
- Client surfaces in `admin/` and `storefront/`

## Verification
- Unit tests: `cargo test -p rustok-party --lib`
- Module contract verification: `cargo xtask module validate party`

## Related Documents
- [Implementation Plan](./implementation-plan.md)
- [Module Platform Overview](../overview.md)
- [Backend Implementation Guide](../../../backend/module-backend-implementation.md)

## Related Documentation
- Documentation maps and registry contracts are centralized in `docs/modules/`.
