# FORUM-26G topic reading posting facts

Status: source-ready / unvalidated; source-ready / maintainer execution pending.

This owner note records the source boundary required by the static contract. Runtime execution, compiled verification and admission remain maintainer-owned; no runtime result is claimed here.

## Source contract

Tests, Cargo, formatting, and CI were not run by the implementation agent; this note records source review only.

- # FORUM-26G topic reading posting facts
- source-ready / unvalidated
- authoritative source is `forum_topic_read_states`
- lifetime reading-ledger count
- empty exact-user ledger is authoritative `0`
- does not join the current topic visibility scope
- Storage failure returns retryable `Unavailable`
- requires the exact user actor
- registers three unique owner providers
- does not expose topic identities
- `forum_user_stats`
- no shared distributed rate-limit reservation
- next bounded FORUM-26 slice
- were not run by the implementation agent
