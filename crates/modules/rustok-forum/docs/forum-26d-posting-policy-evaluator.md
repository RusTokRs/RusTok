# FORUM-26D posting policy evaluator

Status: source-ready / unvalidated; source-ready / maintainer execution pending.

This owner note records the source boundary required by the static contract. Runtime execution, compiled verification and admission remain maintainer-owned; no runtime result is claimed here.

## Source contract

- # FORUM-26D posting policy evaluator
- source-ready / unvalidated
- caller cannot omit a required fact
- Every required fact is resolved before a partial policy decision
- FORUM_POSTING_POLICY_PRECEDENCE
- conservative retry hint
- Shared rate limiting still owns distributed reservation
- `forum_user_stats` is not imported or read
- no duplicate-content hash
- no external or AI scoring call
- next bounded FORUM-26 slice
- were not run by the implementation agent
