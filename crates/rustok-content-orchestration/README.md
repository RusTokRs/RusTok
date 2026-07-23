# rustok-content-orchestration

## Purpose

`rustok-content-orchestration` owns the cross-module bridge implementation for content conversion workflows that span `rustok-content`, `rustok-blog`, `rustok-forum`, `rustok-comments`, and `rustok-taxonomy`.

## Responsibilities

- Implement the `rustok_content::ContentOrchestrationBridge` contract for blog/forum conversion flows.
- Keep cross-module data movement, taxonomy mapping, and canonical URL hand-off
  outside `apps/server`.
- Reject Blog-to-Forum or Forum-to-Blog conversion when comments/replies are
  present until Forum adopts the same canonical richtext contract. The bridge
  must not copy canonical Comments JSON into a legacy selectable-format field
  or silently flatten Forum Markdown.
- Provide host-neutral runtime construction helpers for the server host to register in its own runtime store.

## Entry points

- `build_content_orchestration_service(db, event_bus)`
- `content_orchestration_from_shared(shared)`
- `graphql::ContentOrchestrationMutation`

The real bridge is compiled only when the crate features `mod-content`, `mod-blog`, `mod-forum`, and `mod-comments` are all enabled. Reduced builds get a no-op shared service constructor.

## Interactions

- Consumes orchestration contracts from `rustok-content`.
- Reads and writes owner-owned storage through `rustok-blog`, `rustok-forum`, `rustok-comments`, and `rustok-taxonomy` entities.
- Uses `rustok-outbox::TransactionalEventBus` supplied by the host.
- Is initialized by `apps/server`, but `apps/server` must not own the bridge implementation.
- Uses no host-framework runtime types; host wiring passes explicit DB, event bus and GraphQL data handles.
