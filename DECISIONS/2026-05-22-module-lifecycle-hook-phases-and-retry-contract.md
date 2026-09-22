# 2026-05-22 — Module lifecycle hook phases and retry contract

## Status

Accepted.

## Context

The lifecycle control-plane/module lifecycle must exclude partial rollback
(when the `enabled` flag was already changed and the hook failed) and have a unified recovery contract.
Without an explicit phase model, admin/runtime surfaces interpret lifecycle hook errors differently,
and the `module_operations` journal cannot be reliably used for retry/compensation.

## Decision

- Lifecycle toggle is fixed as a state machine with phases `validated -> running -> committed -> failed`.
- `pre_enable` / `pre_disable` are executed before the tenant state commit. A pre-hook error:
  - does not change the effective module state;
  - completes the operation as `failed` with diagnostics.
- `post_enable` / `post_disable` are executed after a successful commit of tenant state and are considered
  idempotent side-effects.
- A post-hook error does not roll back the committed state. Instead of rollback, a retryable issue is created in
  `module_operations` (via status/details), so recovery proceeds through retry/compensation,
  not through an implicit state rewind.
- Legacy `on_enable` / `on_disable` are treated as a compat pre-hook layer until full cutover to
  the explicit pre/post API.

## Consequences

- GraphQL/SSR surfaces must display a unified error taxonomy: pre-hook failures relate to
  user-facing lifecycle validation failures, post-hook failures are reflected as retryable operations.
- Operational recovery relies on the journal (`correlation_id`, status/details) and becomes
  reproducible without hidden side-effect rollbacks.
- Module owners adding side-effects to hooks must ensure idempotency of the post-phase
  and document retry-safe behavior.
