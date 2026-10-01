# Comments Thread API Contract

- Date: 2026-09-23
- Decision status: Accepted
- Implementation status: Implemented
- Owners: Comments and Blog module owners
- Extends: None
- Supersedes: None
- Superseded by: None

## Context

Blog's Comments capability was lifecycle-optional but the Blog crate still compile-linked the implementation crate. That violated the reduced-build boundary: an optional capability should not force its provider implementation into a consumer build.

`CommentStatus` and several comment DTOs also participate directly in Comments persistence, so extracting them wholesale would leak SeaORM/domain ownership into the neutral consumer contract.

## Decision

Introduce `rustok-comments-api` as the neutral cross-owner contract.

The API crate owns the transport-neutral `CommentsThreadPort` and consumer-facing command/read DTOs. The Comments implementation keeps its SeaORM persistence/domain DTOs private to the owner and converts explicitly at the port boundary.

Blog depends only on `rustok-comments-api`. The host still owns provider selection and may publish either the in-process or remote implementation through `ModuleRuntimeExtensions`. Blog consumers resolve the optional provider from runtime data and degrade through `CommentsUnavailable` instead of constructing a database/event-bus fallback.

The Comments owner implements every API operation, including the explicit public projection. The public projection stays a dedicated owner method and never delegates to the authenticated list operation. Runtime provider absence is represented by the host capability boundary, not by a default trait stub.

## Sources of truth and ownership

- Canonical owner of comments data and lifecycle: `rustok-comments`.
- Canonical contract for consumer integration: `rustok-comments-api`.
- Consumer: `rustok-blog` depends only on `rustok-comments-api`.

## Invariants

1. Blog has no build-time dependency on `rustok-comments`.
2. Comments persistence enums and SeaORM entities remain owned by Comments.
3. `PortContext` and `PortError` remain the shared policy/error boundary.
4. Every provider operation is mandatory in the neutral trait and retains the existing tenant, actor, deadline, policy, and idempotency semantics.
5. Public comments are served only through the explicit safe projection.
6. Missing provider capability affects only comment integration; Blog post publication remains available.
7. Remote transport remains an implementation detail of Comments; the consumer only sees the port.
8. Rollback failures are observable and never silently discarded.

## Non-goals

- Altering the underlying comment storage schema or workflow.
- Merging blog post authoring into comments management.

## Data, transaction, and concurrency boundary

Comments persistence, transactions, and optimistic concurrency remain owned within `rustok-comments`. Consumers never participate directly in comments database transactions.

## Context dimensions

Tenant, channel, locale, and principal are passed via canonical `PortContext`.

## Events and projections

Comment events remain owned and published by `rustok-comments`. Cross-module projections consume events via transactional outbox.

## Failure semantics

Missing or unavailable comments provider results in typed `CommentsUnavailable` port error; consumers fail closed or degrade gracefully without panicking.

## Migration and cutover

Direct crate dependency from `rustok-blog` to `rustok-comments` replaced with dependency on `rustok-comments-api`.

## Alternatives considered

- Direct cross-module crate dependency: rejected due to violating reduced-build and modular boundary isolation.
- Merging comments into core: rejected because comments is a domain module.

## Verification

Source-level guards and architecture boundary tests verify that `rustok-blog` depends only on `rustok-comments-api`.

## Consequences

Reduced Blog builds can omit the Comments implementation while retaining the stable type/port contract. Hosts that include Comments compose its implementation separately. The explicit conversions prevent persistence concerns from crossing the owner boundary.
