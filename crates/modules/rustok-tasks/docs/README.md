# Team Issues & Tasks Module Architecture Contract

## Purpose
The `tasks` module provides canonical Team Issues & Tasks domain functionality for RusToK.

## Responsibility Zone
- Sovereign domain model and persistence for tasks
- Typed events published to the transactional outbox
- RBAC permissions guard: `tasks:read`, `tasks:write`, `tasks:admin`

## Integration
- Core runtime registration in `apps/server` via `RusToKModule`
- Event bus subscription and emission via `rustok-outbox`
- Full-text search discovery projection via `rustok-core`
- Client surfaces in `admin/` and `storefront/`

## Verification
- Unit tests: `cargo test -p rustok-tasks --lib`
- Module contract verification: `cargo xtask module validate tasks`

## Related Documents
- [Implementation Plan](./implementation-plan.md)
- [Module Platform Overview](../overview.md)
- [Backend Implementation Guide](../../../backend/module-backend-implementation.md)

## Related Documentation
- Documentation maps and registry contracts are centralized in `docs/modules/`.
