# Implementation plan for `rustok-core`

Status: foundation crate serves as shared contract layer; sweep boundary hardening is complete — domain-specific auth logic cleaned out, docs and public surface synchronized.

## Execution checkpoint

- Current phase: quality_backlog_hardening
- Last checkpoint: Neutral `Port*`, permission and locale contracts and all compatibility exports were removed from `rustok-core`; core now consumes them from `rustok-api` and owns only runtime RBAC/security policy.
- Next step: Run the documented module verification gates when compilation is allowed and continue extending targeted coverage around dispatcher latency metric hooks.
- Open blockers: None.
- Hand-off notes for next agent: Update this block after each increment.
- Last updated at (UTC): 2026-07-01T00:00:00Z

## Scope of work

- keep `rustok-core` as the minimally necessary shared foundation layer;
- synchronize typed primitives, validation/security contracts and local docs;
- prevent `rustok-core` from becoming a dumping ground for host- or domain-owned logic.

## Current state

- crate is used as a base dependency for platform and domain modules;
- shared typed contracts and foundation helpers are part of the live surface;
- other modules build their integration contracts on top of `rustok-core`, without spreading base types across the workspace;
- **boundary hardening**: auth module (user entity, repository, service, migrations) removed from `rustok-core` — canonical auth lifecycle lives in `rustok-auth`;
- **port boundary hardening**: `PortContext`/`PortError`/`PortCallPolicy` removed from core and its prelude; canonical contract lives only in `rustok-api`;
- **shared API boundary**: `Permission`/`Action`/`Resource` and locale helpers removed from core; `SecurityContext::try_from_port_context` strictly validates non-system actors, `SecurityActorKind` separates `System`/`User`/`Service`/`Public`, and anonymous storefront reads use `SecurityContext::public_read()` instead of trusted runtime authority;
- **contract sync**: `CRATE_API.md`, `README.md`, `docs/README.md` synchronized with the current public surface;
- **deps cleanup**: `jsonwebtoken` and `argon2` removed from `Cargo.toml` (no longer needed after auth removal);
- **targeted tests**: added `tests/foundation_primitives.rs` with coverage for `UserRole`/`UserStatus` (display, parse, serde), `generate_id`/`parse_id`, locale normalization and field-schema guardrails;
- **security/validation tests**: added `tests/security_validation.rs` with coverage for `SecurityHeaders`, `RateLimiter`, `InputValidator`, `SsrfProtection` and utils (`is_valid_email`, `is_valid_uuid`, `html_escape`, `slugify`);
- **contract tests**: expanded `tests/contract_surface.rs` with checks for absence of auth re-exports and unnecessary auth dependencies in `Cargo.toml`;
- **cache/resilience tests**: added `tests/cache_resilience_contract.rs` with coverage for in-memory cache TTL/invalidation, retry predicates/backoff caps, circuit breaker manual/half-open controls, bulkhead metrics and timeout errors;
- **event observability tests**: added `tests/events_observability_contract.rs` with coverage for saturating backpressure release, warning/critical metrics, EventBus publish/drop stats and MemoryTransport batch stats;
- **dispatcher retry tests**: expanded unit tests `src/events/handler.rs` for retry success, retry exhaustion with `on_error`, no-handler backpressure release and concurrent handler completion release;
- local docs and root `README.md` are maintained as part of the scoped audit path.

## Stages

### 1. Contract stability

- [x] lock `rustok-core` as shared foundation layer;
- [x] keep typed primitives and shared helpers outside host/domain buckets;
- [x] maintain sync between public surface, compatibility exports and module metadata.

### 2. Boundary hardening

- [x] continue cleaning domain-specific logic from foundation layer;
- [x] move shared primitives here only when real cross-module necessity arises;
- [x] cover new foundation contracts with targeted tests and compatibility checks.

### 3. Operability

- [x] document foundation contract changes simultaneously with changing runtime surface;
- [x] keep local docs and `README.md` synchronized;
- [x] update consumer-module docs if base typed contracts change.

## Verification

- contract tests cover all public use-cases
- `cargo xtask module validate core`
- `cargo xtask module test core`
- targeted tests for primitives, validation, security, permissions, rt_json sanitization, cache/resilience contracts, event observability contracts, dispatcher retry/backpressure-release contracts and compatibility exports

## Update rules

1. When changing foundation contract, update this file first.
2. When changing public/runtime surface, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata, synchronize `rustok-module.toml`.
4. When changing shared contracts, update related consumer docs where it affects live behavior.


## Quality backlog

- [x] Update test coverage for key module scenarios — added permission/rt_json/cache/resilience/event-observability and dispatcher retry/backpressure-release contract tests.
- [x] Verify completeness and currency of `README.md` and local docs — README/docs remain aligned with foundation-only surface; no runtime changes introduced.
- [x] Lock/update verification gates for current module state — documented gates preserved; execution deferred due to compilation prohibition in this iteration.
