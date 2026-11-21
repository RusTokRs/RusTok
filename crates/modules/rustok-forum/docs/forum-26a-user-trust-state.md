# FORUM-26A user trust state

Status: source-ready / unvalidated; source-ready / maintainer execution pending.

This owner note records the source boundary required by the static contract. Runtime execution, compiled verification and admission remain maintainer-owned; no runtime result is claimed here.

## Source contract

Tests, Cargo, formatting, and CI were not run by the implementation agent; this note records source review only.

- # FORUM-26A user trust state
- source-ready / unvalidated
- Absence of a current row means trust level `0`
- `AFTER INSERT` triggers materialize its matching current state
- `forum_user_stats` remains an activity-counter projection
- no trust facts adapter
- no automatic posting-policy evaluator
- The next bounded slice should publish a read-only trust facts adapter
- canonical `crates/modules/rustok-forum/docs/implementation-plan.md` is intentionally not rewritten
- were not run by the implementation agent
