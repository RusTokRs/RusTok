# FORUM-20AY moderation audience policy

Status: source-ready / unvalidated.

## Delivered

- Five normalized Forum-owned tables persist one optional category-local moderation audience layer plus typed role, Channel, Groups, and explicit allow/deny relations.
- The policy is independent from content visibility, topic-create eligibility, and reply-create eligibility.
- `ForumCategoryModerationAudiencePolicyService` exposes managed `get` and atomic replacement `set` commands under `forum_categories:manage`.
- Empty constraints clear only the target category layer and restore inherited root-to-category moderation policy.
- Every public topic and reply moderation command resolves the exact tenant-scoped target category and evaluates every inherited layer before opening its write transaction.
- Existing context-free commands preserve compatibility for unrestricted, role-only, and explicit-user decisions. Unresolved trust, Channel, or Groups selectors require an exact `PortContext` and an injected `SharedForumAudienceFactsPort`.
- `ModerationService::with_audience_facts` and context-aware command variants publish the owner seam needed for transport composition.
- Moderator use of `mark_solution` and `clear_solution` is audience-gated. The exact tenant-scoped topic author remains independently authorized to select or clear a solution without receiving moderator privileges.
- Denied or unresolved decisions occur before topic/reply status, pin, lock, counter, user-stat, solution, journal, and outbox writes.
- PostgreSQL and SQLite enforce tenant/category ownership, typed values, immutable rows, and bounded direct channel/group/allow/deny inserts.
- PostgreSQL managed replacement and direct bounded inserts use the same category moderation advisory lock key.

## Boundary

- GraphQL, REST, OpenAPI, quote, and moderation DTOs are unchanged.
- Existing solution transports were composed in `FORUM-20AZ`; moderation owner
  methods without an existing public route remain transport-neutral.
- No Forum trust owner state or trust facts adapter was added by this slice.
  Authoritative Forum trust is now host-composed through
  `ForumUserTrustAudienceFactsPort` and is never derived from `forum_user_stats`.
- No topic-local moderation layer, report queue, restriction, audit, or anti-spam policy is added.
- No dependency or host/server source changes are included.

## Canonical plan synchronization

Resolved by `FORUM-20BA`. The canonical ledger records category moderation
audience persistence/enforcement and the existing solution transport composition
through `FORUM-20AZ`.

## Validation status

The following commands are source-ready but were not run by the implementation agent:

```text
cargo test -p rustok-forum --test moderation_audience_policy_sqlite -- --nocapture
node scripts/verify/verify-forum-moderation-audience-policy.mjs
cargo xtask module validate forum
```

Tests, Cargo commands, formatting, verifier execution, workflows, and CI remain the maintainer's responsibility for this slice.
