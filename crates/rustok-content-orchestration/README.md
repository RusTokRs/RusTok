# rustok-content-orchestration

## Purpose

`rustok-content-orchestration` owns the cross-module bridge implementation for content conversion workflows that span `rustok-content`, `rustok-blog`, `rustok-forum`, `rustok-comments`, and `rustok-taxonomy`.

## Responsibilities

- Implement the `rustok_content::ContentOrchestrationBridge` contract for blog/forum conversion flows.
- Keep cross-module data movement, comment/reply migration, taxonomy mapping, and canonical URL hand-off outside `apps/server`.
- Provide runtime initialization helpers for the server host shared store.

## Entry points

- `init_content_orchestration(ctx, event_bus)`
- `content_orchestration_from_context(ctx)`

The real bridge is compiled only when the crate features `mod-content`, `mod-blog`, `mod-forum`, and `mod-comments` are all enabled. Reduced builds get a no-op initializer.

## Interactions

- Consumes orchestration contracts from `rustok-content`.
- Reads and writes owner-owned storage through `rustok-blog`, `rustok-forum`, `rustok-comments`, and `rustok-taxonomy` entities.
- Uses `rustok-outbox::TransactionalEventBus` supplied by the host.
- Is initialized by `apps/server`, but `apps/server` must not own the bridge implementation.
