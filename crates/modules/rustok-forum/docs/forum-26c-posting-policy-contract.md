# FORUM-26C posting policy contract

Status: source-ready / unvalidated; source-ready / maintainer execution pending.

This owner note records the source boundary required by the static contract. Runtime execution, compiled verification and admission remain maintainer-owned; no runtime result is claimed here.

## Source contract

- # FORUM-26C posting policy contract
- source-ready / unvalidated
- Every declared fact must be represented exactly once
- `indeterminate` from `denied`
- does not expose an evaluator, service or owner port
- `forum_user_stats` is not read
- no shared distributed rate-limit call
- no duplicate-content hashing
- no external or AI spam-scoring call
- next bounded FORUM-26 slice
- canonical `crates/modules/rustok-forum/docs/implementation-plan.md` is not replaced
- were not run by the implementation agent
