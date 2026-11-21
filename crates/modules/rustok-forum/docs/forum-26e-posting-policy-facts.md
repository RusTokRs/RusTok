# FORUM-26E posting policy facts

Status: source-ready / unvalidated; source-ready / maintainer execution pending.

This owner note records the source boundary required by the static contract. Runtime execution, compiled verification and admission remain maintainer-owned; no runtime result is claimed here.

## Source contract

- # FORUM-26E posting policy facts
- source-ready / unvalidated
- Providers are registered uniquely
- A missing provider is represented
- validation, forbidden, conflict and invariant failures propagate
- ForumPostingTrustFactPort
- initial supported composition profile
- does not evaluate or enforce policy
- `forum_user_stats` is not imported or read
- no shared rate-limit reservation
- next bounded FORUM-26 slice
- were not run by the implementation agent
