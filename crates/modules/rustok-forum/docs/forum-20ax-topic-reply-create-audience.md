# FORUM-20AX topic-local reply-create audience narrowing

Status: source-ready / unvalidated.

## Delivered

- Forum stores an optional normalized reply-create audience layer for each tenant/topic independently from topic visibility and inherited category reply-create storage.
- Managed `get` and atomic replacement `set` require `forum_topics:manage`; an empty constraint set clears only the topic layer.
- Effective reply authorization is the conjunction of every non-empty root-to-category layer followed by the optional topic layer. The topic can narrow but never broaden a category rule.
- The existing reply authorization service attributes internal denial to either the exact category layer or the topic layer while retaining one generic public denial.
- Legacy and inline-quote reply owner methods, and therefore the already-composed GraphQL and REST transports, inherit the final topic check without transport or DTO changes.
- PostgreSQL and SQLite enforce tenant/topic composite ownership, typed roles/effects, non-nil group/user IDs, immutable rows, and bounded direct channel/group/allow/deny inserts.

## Boundary

- Topic reply-create policy does not read or mutate `forum_topic_audience_*` visibility rows.
- No GraphQL, REST, OpenAPI, quote, or owner DTO fields changed.
- No dependency or host/server composition changed.
- No Forum trust state or trust facts adapter was added by this slice.
  Authoritative Forum trust is now supplied by the Forum owner through
  `ForumUserTrustAudienceFactsPort`; activity counters remain unrelated to trust.
- Moderation audience policy was a separate next slice and was subsequently
  delivered in `FORUM-20AY`.

## Canonical plan synchronization

Resolved by `FORUM-20BA`. The canonical ledger now removes topic-local reply
narrowing from remaining scope and records the later moderation audience chain
through `FORUM-20AZ`.

## Validation status

The following commands are source-ready but were not run by the implementation agent:

```text
cargo test -p rustok-forum --test topic_reply_create_audience_policy_sqlite -- --nocapture
node scripts/verify/verify-forum-topic-reply-create-audience-policy.mjs
node scripts/verify/verify-forum-reply-create-audience-enforcement.mjs
node scripts/verify/verify-forum-reply-create-audience-transport-composition.mjs
cargo xtask module validate forum
```

Tests, Cargo commands, formatting, verifier execution, workflows, and CI remain the maintainer's responsibility for this slice.
