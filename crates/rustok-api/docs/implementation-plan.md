# `rustok-api` - Implementation Plan

Status: shared host/API layer already serves as the foundation for `apps/server` and
module-owned transport adapters; the main task is to prevent it from growing into a
parallel application layer.

## Execution checkpoint

- Current phase: neutral contract ownership hardening
- Last checkpoint: `Port*`, permission and locale contracts moved into `rustok-api`; API no longer depends on core in any feature, core compatibility exports were deleted, and outbox Loco composition moved to `rustok-outbox::loco`.
- Next step: Keep new module ports on `rustok_api::ports` and reject runtime-specific dependencies in the default contract surface.
- Open blockers: None.
- Hand-off notes for next agent: Update this block after each increment.
- Last updated at (UTC): 2026-07-01T00:00:00Z

## Scope of work

- maintain `rustok-api` as the shared web/API adapter foundation;
- synchronize request/auth/tenant/channel/GraphQL/port contracts and local docs;
- prevent module-specific business logic from being pulled into the shared API layer.

## Current state

- crate already provides shared request/auth/tenant/channel contexts and GraphQL helpers;
- `PortContext`/`PortError` set the shared baseline for transport-agnostic ports, and `PortCallPolicy` fixes reusable read/write/event-replay/best-effort enforcement without module-specific logic; `rustok-region`, tenant, channel, product, customer, media, workflow, RBAC, tax, fulfillment, payment, pricing, cart, inventory, comments, search, order, index, email delivery, outbox relay and page-builder publish paths already consume the shared policy baseline (`PortCallPolicy::read()` for read projections, `PortCallPolicy::write()` for write control);
- default and `server` feature sets own neutral API contracts without dependency on `rustok-core`; runtime RBAC/security lives in core, which depends on API;
- `apps/server` remains the composition root above this layer, not a second parallel shared API framework;
- module transport adapters use `rustok-api` for shared host/API contracts without duplicating them locally.

## Stages

### 1. Contract stability

- [x] lock `rustok-api` as the shared host/API layer;
- [x] maintain reusable request/auth/channel/GraphQL/port contracts outside `rustok-core`;
- [~] maintain sync between public surface, host wiring and local docs; (updated: UI helper ownership moved to `rustok-ui-core`, `leptos-ui-routing` and `rustok-ui-i18n`)

### 2. Boundary hardening

- [~] continue extracting truly shared transport/port helpers from host/module-specific layers; (continued: neutral port context/error primitives, port call policies, typed error constructors and expanded multi-module read/write-port consumer migration)
- [ ] do not pull module-owned resolvers and controllers here;
- [ ] cover new shared contracts with targeted compile/tests when changing surface.

### 3. Operability

- [~] document host/API contract changes simultaneously with runtime surface changes; (updated for shared write-policy migration across inventory/comments/fulfillment/order/payment/page-builder and previous read-policy cleanup)
- [~] keep local docs and `README.md` synchronized; (updated for shared write-policy migration across inventory/comments/fulfillment/order/payment/page-builder and previous read-policy cleanup)
- [ ] update consumer-module docs if shared transport expectations change.

## Verification

- structural verification for local docs and host/API boundary;
- targeted compile/tests when changing shared request/auth/channel/GraphQL contracts;
- docs sync for `apps/server` and module-owned transport crates.

## Update rules

1. When changing a shared host/API contract, first update this file.
2. When changing the public/runtime surface, synchronize `README.md` and `docs/README.md`.
3. When changing consumer expectations, update related host/module docs.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and relevance of `README.md` and local docs.
- [ ] Lock/update verification gates for the current module state.
