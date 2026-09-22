# ADR: Port boundary for content orchestration

- Status: Accepted
- Date: 2026-03-28

## Context

After the storage split, `rustok-blog`, `rustok-forum`, `rustok-pages` and `rustok-comments`
own their own tables, but continue to depend on `rustok-content` as a shared helper
layer for locale, rich-text, slug policy, and other common rules.

The old `ContentOrchestrationService` remained tied to `NodeService` and shared `nodes`,
which broke the new architectural boundary:

- orchestration depended on legacy storage topology;
- `topic ↔ post` transfer was implemented as rebinding shared children;
- directly rewriting `rustok-content` to depend on `blog/forum/comments` would create
  a cyclic dependency graph.

## Decision

`ContentOrchestrationService` is moved to a port boundary:

- `rustok-content` keeps orchestration state, idempotency, audit, and event publication;
- domain conversion is moved to `ContentOrchestrationBridge`;
- runtime adapters that know about `blog/forum/comments` persistence must live outside
  the shared helper layer and implement `ContentOrchestrationBridge`;
- the runtime adapter and conversion GraphQL mutation surface live in
  `rustok-content-orchestration`; the canonical-route query remains in `rustok-content`;
- content GraphQL entity dataloaders for `nodes`, `node_translations` and `bodies`
  also live in `rustok-content`; `apps/server` may register them but does not own them;
- `apps/server` only connects these GraphQL roots and does not own resolvers, DTOs, or
  concrete connection types of modules;
- `rustok-content` no longer has the right to directly transfer shared `node` children between
  parents and must not treat `nodes` as the canonical source of truth for conversion flows.

## Consequences

Positives:

- removes the tight coupling of orchestration from legacy `NodeService`;
- preserves the role of `rustok-content` as a shared helper/orchestration layer without cycles;
- RBAC, idempotency, audit, and event contract remain centralized.

Negatives:

- a separate adapter layer is needed for real runtime conversion flows;
- host schema composition depends on feature-gated owner/support GraphQL entrypoints
  and owner-owned dataloader types;
- mapping rules `blog comments ↔ forum replies` must now be explicitly described and implemented
  in the integration adapter, rather than "magically" through shared topology.

## What is considered unacceptable

- returning `ContentOrchestrationService` to shared `NodeService`;
- tying new orchestration logic to `nodes`/`node_translations` as a source of truth;
- adding direct dependencies `rustok-content -> rustok-blog/rustok-forum/rustok-comments`
  if this closes a dependency cycle.
- returning conversion GraphQL resolver/DTO, content entity dataloaders, or module-specific concrete connection types
  to `apps/server`.
