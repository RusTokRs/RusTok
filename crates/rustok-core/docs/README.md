# Documentation `rustok-core`

`rustok-core` is the base foundation crate of the platform. It defines shared typed
contracts, errors, security primitives, content helpers and other invariants that
the remaining RusToK modules rely on.

## Purpose

- publish the canonical shared foundation contract for platform and domain modules;
- keep typed primitives and basic invariants outside host- and domain-specific code;
- reduce duplication of cross-module contracts without turning `rustok-core` into a runtime bucket for any logic.

## Responsibilities

- typed primitives and shared value objects (e.g., `UserRole`, `UserStatus` for RBAC);
- basic error/validation helpers and security contracts;
- foundation event contracts: canonical event re-exports, in-memory transport, bus stats, observability for backpressure, dispatcher retry semantics and dispatch delay hooks;
- reject invalid root envelopes at the in-memory event-bus ingress through the canonical `rustok-events` validation contract;
- content/rich-text helper contracts used by multiple modules (`rt_json`, `grapesjs`, `content_format`);
- flex/custom-fields schema contracts (`field_schema`);
- compatibility re-exports and shared API surface for the foundation layer;
- absence of domain-owned runtime orchestration and transport-specific logic.
- absence of neutral module-port DTO/error contracts: `Port*` belong to `rustok-api` and are not re-exported from core/prelude.

## Integration

- used by virtually all `rustok-*` crates as a foundation dependency;
- `apps/server` and runtime modules depend on typed contracts, but should not pull shared logic back into the host layer;
- `rustok-events`, `rustok-rbac`, `rustok-content` and other foundation/domain crates must remain on top of `rustok-core`, not the other way around;
- `rustok-auth` owns the canonical auth lifecycle; `rustok-core` does not duplicate auth-specific services, repositories or migrations;
- any new cross-module primitives should go here only if they are truly shared and do not belong to a single bounded context.

## Verification

- `cargo xtask module validate core`
- `cargo xtask module test core`
- targeted tests for typed primitives, validation helpers, security contracts, event observability contracts, dispatcher retry/latency contracts and compatibility exports

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
