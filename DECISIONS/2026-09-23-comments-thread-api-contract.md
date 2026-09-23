# Comments Thread API Contract

## Status

Accepted and implemented on 2026-09-23.

## Context

Blog's Comments capability was lifecycle-optional but the Blog crate still compile-linked the implementation crate. That violated the reduced-build boundary: an optional capability should not force its provider implementation into a consumer build.

`CommentStatus` and several comment DTOs also participate directly in Comments persistence, so extracting them wholesale would leak SeaORM/domain ownership into the neutral consumer contract.

## Decision

Introduce `rustok-comments-api` as the neutral cross-owner contract.

The API crate owns the transport-neutral `CommentsThreadPort` and consumer-facing command/read DTOs. The Comments implementation keeps its SeaORM persistence/domain DTOs private to the owner and converts explicitly at the port boundary.

Blog depends only on `rustok-comments-api`. The host still owns provider selection and may publish either the in-process or remote implementation through `ModuleRuntimeExtensions`. Blog consumers resolve the optional provider from runtime data and degrade through `CommentsUnavailable` instead of constructing a database/event-bus fallback.

The Comments owner implements the API trait. Its public projection stays a dedicated owner method and never delegates to the authenticated list operation.

## Invariants

1. Blog has no build-time dependency on `rustok-comments`.
2. Comments persistence enums and SeaORM entities remain owned by Comments.
3. `PortContext` and `PortError` remain the shared policy/error boundary.
4. Every provider operation retains the existing tenant, actor, deadline, policy, and idempotency semantics.
5. Public comments are served only through the explicit safe projection.
6. Missing provider capability affects only comment integration; Blog post publication remains available.
7. Remote transport remains an implementation detail of Comments; the consumer only sees the port.
8. Rollback failures are observable and never silently discarded.

## Consequences

Reduced Blog builds can omit the Comments implementation while retaining the stable type/port contract. Hosts that include Comments compose its implementation separately. The explicit conversions prevent persistence concerns from crossing the owner boundary.

## Verification

Source-level guards and evidence were updated for the extracted API boundary. Maintainer runtime evidence, build, gatekeeper, and automated tests remain unrun by the agent; the repository owner executes those checks.
