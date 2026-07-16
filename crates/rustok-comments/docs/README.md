# Documentation of the `rustok-comments` module

`rustok-comments` is a domain module for classic comments outside the forum.

## Purpose

- provide a separate storage boundary for comments on blog posts and other opt-in non-forum entities;
- remove comments from the shared `content` storage model;
- establish that `comments` and `forum replies` are different domain entities;
- prepare a modular foundation for future conversion flows between `blog` and `forum` via orchestration.

## Scope

- own generic comment thread/comment/body storage and moderation status policy;
- expose module-owned service and admin moderation UI contracts;
- support `rustok-blog` and future explicit opt-in commentable surfaces through public contracts;
- exclude forum replies and default pages integration from the comments storage boundary.

## Responsibilities

- `rustok-comments` owns only the generic comments domain, its schema and service-level contracts;
- `rustok-forum` continues to own `forum_topics` and `forum_replies`;
- `rustok-content` remains a shared library + orchestration layer and should not become the storage owner for comments again;
- conversion flow `post + comments -> topic + replies` and vice versa must live in orchestration, not through a shared table or live sync.

## Current status

- the module is registered in the workspace, `modules.toml` and optional server wiring;
- module-owned schema `comment_threads`, `comments`, `comment_bodies` is implemented;
- `rustok-blog` has already been migrated to `rustok-comments` for the comment read/write path;
- shared rich-text/body-format and locale fallback contract are aligned with `rustok-content`;
- thread status contract is no longer decorative: `closed` actually blocks new
  create-path, and terminal comment statuses (`spam`, `trash`) require moderation scope;
- runtime-composed public-port create/delete publish `comment.created` and
  `comment.deleted` through the owner transaction outbox; Blog projection and
  live delivery evidence remain pending;
- the module now publishes `rustok-comments-admin` as a module-owned Leptos moderation UI;
- for operator-facing read/write path, service-level methods `list_threads`,
  `get_thread_detail`, `set_thread_status` and `set_comment_status` have been added;
- the product decision on `pages <-> comments` is finalized: `rustok-pages` has no default
  integration with `rustok-comments`, and future page-like discussion surfaces are only possible
  as explicit opt-in.

## Integration

- `rustok-blog` has already been migrated to `rustok-comments` for the live comment read/write path;
- moderation UI is published as a module-owned Leptos surface `rustok-comments-admin`;
- runtime transport adapters and host wiring remain in `apps/server`, while module-owned admin moderation UI goes through its own `admin/src/transport/` facade; domain logic and moderation contract belong to the module;
- future integrations for page-like surfaces must be formalized as an explicit opt-in contract.

## Module-owned admin UI and transport rule

- `rustok-comments-admin` is mounted in Leptos Admin as a module-owned UI at `/modules/comments`.
- The internal data-layer for the moderation surface is built through `admin/src/transport/mod.rs` facade and `admin/src/transport/native_server_adapter.rs` native `#[server]` calls over `CommentsService`.
- The native admin transport consumes host-provided `rustok_api::HostRuntimeContext` for DB access and must not depend on a host-wide `AppContext`.
- Selected-thread and locale route/query policy belongs to `admin/src/core.rs` and uses shared `UiRouteQueryUpdate`; the Leptos adapter only applies the ready host writer update.
- Fast boundary guardrail: `npm run verify:comments:admin-boundary` checks the FFA split and documented native-only transport exception.
- A separate GraphQL/REST fallback for this UI is not added: `rustok-comments` did not have its own legacy transport surface, and this is a documented exception from the general dual-path rule.
- The existing integration `rustok-blog -> rustok-comments` is not changed by this.

## Status contract

- `comment_threads.status = open|closed` only controls the acceptance of new
  comments; a closed thread remains readable but does not accept new entries;
- the normal create-path only allows `pending|approved`;
- `spam|trash` are considered moderation statuses and require `comments:moderate`
  or `comments:manage`;
- thread status changes are made through the service-level
  `set_thread_status_for_target`, not by direct DB write from the transport layer.

## Observability

- service entry-points `create_comment`, `get_comment`, `update_comment`,
  `delete_comment`, `list_comments_for_target` write
  `rustok_module_entrypoint_calls_total{module="comments",path="library"}`;
- service errors are classified into low-cardinality `database`,
  `not_found`, `forbidden`, `validation` and written to
  `rustok_module_errors_total`;
- latency/error per operation is written through
  `rustok_span_duration_seconds{operation="comments.*"}` and
  `rustok_spans_with_errors_total`;
- the bounded read-path `list_comments_for_target` writes
  `read_path_requested_limit/effective_limit/returned_items/query_duration/query_rows`
  with `surface="library"` and `path="comments.list_comments_for_target"`.

## Next steps

- if commentable page-like surfaces appear later, describe them with a separate spec/ADR, rather than
  extending the current pages contract by default.


## Operational alerts and operator playbook

- `rustok_module_errors_total{module="comments",kind="database"}` — page-now alert: this is a runtime/storage incident, not a normal moderation rejection.
- `rustok_module_errors_total{module="comments",kind="conflict"}` on `comments.create_comment` should normally only be explained by `CommentThreadClosed`; if a spike occurs without a conscious close-thread action, first check target binding and transport/client drift.
- `rustok_module_errors_total{module="comments",kind="forbidden"}` on create/update/delete and `set_thread_status_for_target` is a warning-level signal for RBAC/moderation drift; first verify the caller's effective permissions.
- `rustok_module_errors_total{module="comments",kind="validation"}` is acceptable for normal bad payloads, but repeated attempts to write `spam|trash` without moderation scope should be treated as a client/moderation UX regression.
- For `comments.list_comments_for_target`, check stage-level `query_duration/query_rows` (`comment_threads.lookup`, `comments.page`, `comment_bodies.batch`) and budget-metrics `requested_limit/effective_limit/returned_items` together to separate DB latency from over-requesting callers.

Operator action plan:

1. First classify the spike by `kind`: `database`, `conflict`, `forbidden` or `validation`.
2. For `conflict`, check the target thread state in `comment_threads` and recent `set_thread_status_for_target` calls; a closed thread should fully explain the reject pattern.
3. For `forbidden`, check recent RBAC changes and caller scopes: `spam|trash` and thread status changes should only come from moderation-capable callers.
4. For latency without an error spike, first analyze read-path stages rather than immediately escalating a general DB incident.
5. For sustained `database` errors, switch to the general DB/runtime incident flow: connections, recent deploy, migration drift, query pressure.

## Verification

- `cargo xtask module validate comments`
- `cargo xtask module test comments`
- `node scripts/verify/verify-comments-admin-boundary.mjs`
- targeted tests for moderation/status contract, module-owned admin UI and blog integration path

## Related documents

- [Implementation plan](./implementation-plan.md)
- [README crate](../README.md)
- [ADR: `rustok-pages` does not get default integration with `rustok-comments`](../../../DECISIONS/2026-03-29-pages-comments-no-default-integration.md)
- [Documentation map](../../../docs/index.md)
