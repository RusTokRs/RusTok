# FORUM-26E posting policy facts

Status: source-ready / unvalidated.

## Delivered

- `ForumPostingPolicyOwnerFactPort` is a public owner-fact SPI for one exact posting-policy fact, tenant, user and action.
- `ForumPostingPolicyFactsComposer` normalizes the composition request and rules, derives the exact required fact set from `ForumPostingPolicyRules` plus the action, and resolves only those facts.
- Providers are registered uniquely by `ForumPostingPolicyFactKind`. Duplicate ownership fails composition setup instead of selecting an arbitrary provider.
- Every owner call receives the same exact user-scoped `PortContext`. The composer requires read deadline semantics, an exact tenant string and an exact `PortActorKind::User` UUID before any provider is invoked.
- Topic-create, reply-create and edit usage facts receive the exact configured observation window. Responses must preserve tenant, user, action, fact kind, typed value kind and requested window.
- A missing provider is represented as `ForumPostingPolicyUnavailableFact` with stable reason `forum.posting_fact.provider_missing` and `retryable=false`. The composer never replaces an undelivered account-age, activity, moderation, reputation or usage fact with zero.
- Provider `Unavailable`, `Timeout` and `NotFound` errors become explicit unavailable facts and preserve owner retryability. Invalid owner error codes use the bounded fallback `forum.posting_fact.provider_error`.
- Provider validation, forbidden, conflict and invariant failures propagate as port errors. They are not hidden as a degraded policy fact.
- `ForumPostingTrustFactPort` bridges the already published authoritative `SharedForumAudienceFactsPort` into `TrustLevel`. It requests trust only, with no Channel or Groups membership candidates, and requires a validated trust response.
- `ForumPostingPolicyFactsComposer::with_trust_audience_facts` provides the initial supported composition profile: authoritative trust plus local candidate link, mention and attachment metrics. Rules requiring any undelivered owner fact still produce an explicit unavailable fact.
- The final result is normalized through the FORUM-26C `ForumPostingPolicyEvaluationInput` contract and can be passed to the FORUM-26D evaluator by a later caller.

## Ownership boundary

This slice composes facts but does not evaluate or enforce policy. `ForumPostingPolicyFactsComposer` never calls `ForumPostingPolicyEvaluator`, writes trust state or invokes a shared rate limiter.

`forum_user_stats` is not imported or read. Its topic/reply/solution counters are not authoritative trust, approved-post, reputation, moderation-history or distributed usage-window facts.

The trust bridge reuses the FORUM-26B audience facts adapter so absent authoritative trust state retains the already documented level `0` behavior. That is an owner-defined trust projection result, not a composer fallback. Every other missing provider remains unavailable rather than zero.

## Initial supported profile

The source-ready profile can compose:

- authoritative Forum trust level;
- candidate link count;
- candidate mention count;
- candidate attachment count.

Candidate metrics are already part of the bounded FORUM-26C request and require no owner port. Account age, reading, approved posts, flags, reputation, moderation history, usage windows and bump age remain unavailable until their named owners are implemented.

## Excluded

- no account-age owner adapter;
- no reading or approved-post owner adapter;
- no active-flag or moderation-history owner adapter;
- no reputation ledger or owner adapter;
- no distributed topic/reply/edit window or bump-age adapter;
- no policy settings persistence or administration;
- no topic/reply/edit/bump owner enforcement;
- no shared rate-limit reservation, commit, release or exact retry calculation;
- no duplicate-content hash or retained fingerprint;
- no external or AI scoring call;
- no trust promotion, demotion or other trust-state write;
- no migration, event, worker, GraphQL, REST, OpenAPI, admin UI, storefront or server runtime publication.

The next bounded FORUM-26 slice should publish the first non-trust authoritative fact owner with clear ownership and degraded behavior. A sensible next candidate is account age from the identity/users owner. Posting owner enforcement and distributed limiter execution should remain separate later slices.

## Canonical plan debt

The canonical `crates/rustok-forum/docs/implementation-plan.md` is not replaced through the GitHub contents API. It exceeds two thousand lines and complete replacement risks unrelated roadmap loss. A safe repository-local edit still needs to mark FORUM-26 `in_progress`, record FORUM-26A-E and retain owner facts, enforcement, duplicate hashing, shared rate limiting and optional external scoring as remaining work.

`CRATE_API.md` is likewise not completely replaced. The public owner-fact SPI, composer and trust bridge are exported from the crate root and recorded by the machine contract and verifier.

## Validation status

The following commands are source-ready but were not run by the implementation agent:

```text
cargo test -p rustok-forum --test posting_policy_facts -- --nocapture
cargo test -p rustok-forum --test posting_policy_evaluator -- --nocapture
node scripts/verify/verify-forum-posting-policy-facts.mjs
node scripts/verify/verify-forum-posting-policy-evaluator.mjs
node scripts/verify/verify-forum-user-trust-audience-facts.mjs
cargo xtask module validate forum
```

Tests, Cargo commands, formatting, verifier execution, workflows and CI remain the maintainer's responsibility for this slice.
