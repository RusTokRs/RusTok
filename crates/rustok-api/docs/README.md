# `rustok-api` Documentation

`rustok-api` is the shared web/API adapter layer of the platform. It holds common
request/auth/tenant/channel/GraphQL contracts needed by the host layer and
module transport adapters, but which should not live in `rustok-core`.

## Purpose

- publish the canonical shared host/API contract;
- keep reusable request, auth, tenant, channel and GraphQL-facing primitives outside `apps/server`;
- give module crates a common transport-adapter foundation without duplicating web-layer contracts;
- publish neutral port primitives (`PortContext`, `PortError`, `PortCallPolicy`) for new and gradually migrated transport-agnostic ports.

## Area of Responsibility

- request context types, transport-agnostic port context/error/policy primitives and auth/tenant/channel host contracts;
- `UiRouteContext`, `UiRouteQueryUpdate`, `normalize_ui_text`, `parse_ui_csv` and other module-agnostic UI host contracts;
- compatibility re-exports for `rustok-ui-i18n` message catalog helpers while consumers migrate to the direct crate;
- GraphQL helper types and error helpers shared across modules;
- reusable read/write/event-replay/best-effort port enforcement without module-specific business logic; consumer migration is anchored in `rustok-region` and continued for tenant, channel, product, customer, media, workflow, RBAC, tax, fulfillment, payment, pricing, cart, inventory, comments, search, order, index, email delivery, outbox relay and page-builder publish paths;
- request-level locale/tenant/channel resolution primitives not belonging to domain crates;
- absence of module-specific resolvers, controllers and business logic.

## Integration

- used by `apps/server` as the shared composition/root API layer;
- module crates may depend on `rustok-api` when their GraphQL/REST adapters live inside the modules themselves;
- default surface does not depend on `rustok-core` and publishes neutral contracts directly;
- `server` feature enables `rustok-core` only for server-side security/permission/auth/request/GraphQL integration;
- outbox-specific Loco wiring belongs to `rustok-outbox::loco`, so `rustok-api` does not depend on outbox runtime;
- must not be duplicated in `apps/server` or in per-module helper crates.

## Verification

- structural verification: local docs and root `README.md` must remain synchronized;
- targeted compile/tests run when shared request/auth/channel/GraphQL/UI contracts change;
- `cargo tree -p rustok-api --no-default-features` must not contain `rustok-core` or `rustok-outbox`;
- changes to the host/API layer must be accompanied by synchronization of consumer docs.
- UI message catalog changes should be made in `rustok-ui-i18n` and verified there.

## Related Documents

- [README crate](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [Platform documentation map](../../../docs/index.md)
