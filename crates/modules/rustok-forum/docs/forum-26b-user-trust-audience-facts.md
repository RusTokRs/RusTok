# FORUM-26B user trust audience facts

Status: source-ready / unvalidated.

## Delivered

- `ForumUserTrustAudienceFactsPort` is a Forum-owned implementation of `ForumAudienceFactsPort` over the authoritative `forum_user_trust_states` projection introduced by FORUM-26A.
- The adapter accepts one normalized tenant/user request under read-only `PortContext` policy and requires the exact requested user actor. Foreign tenants, service actors, public callers and foreign users fail closed before owner reads.
- Missing trust state resolves to trust level `0`. A configured state resolves its stored bounded level `0..100`; invalid stored identity, revision or level is an invariant failure.
- Storage errors are typed retryable unavailability. They are never converted into trust level `0`.
- Channel and Groups dimensions are delegated to the existing host membership provider with `include_trust_level = false`. The delegated response is validated against the exact bounded membership request.
- An actual requested Channel or Groups membership decides the positive-selector union and returns without reading trust storage.
- Trust is read only after a bounded membership provider confirms that no requested membership matched. If the optional membership provider cannot resolve a requested dimension, its retryable error is preserved instead of becoming a false negative.
- The default server composition keeps the historical `ServerForumAudienceFactsPort` call site and publishes a trust wrapper around its Channel/optional Groups provider. Existing GraphQL and REST context composition therefore consumes the same `SharedForumAudienceFactsPort` without new identity fields or routes.

## Ownership boundary

`forum_user_stats` remains an activity-counter projection. The trust facts adapter does not import `UserStatsService`, read the table, infer trust from topic/reply/solution counts, or copy those counters into the facts response.

The adapter reads Forum-owned trust state directly rather than calling the managed `ForumUserTrustService::get` command because audience evaluation is an exact-actor capability read, not a `forum_topics:manage` administration operation. Trust writes and immutable revision history remain exclusively owned by the FORUM-26A service and database guards.

## Excluded

- no migration, trust-state write, revision or idempotency change;
- no automatic trust promotion/demotion or explainable policy evaluator;
- no account-age, reading, approved-content, flag, reputation or moderation-history owner facts;
- no topic/reply posting limit, duplicate-content hash or shared rate-limit execution;
- no GraphQL, REST, OpenAPI, admin UI or public DTO;
- no external or AI scoring;
- no Channel or Groups dependency change.

The next bounded FORUM-26 slice should define the typed explainable evaluation input and decision contract while keeping every unavailable fact explicit. It must not derive policy from `forum_user_stats` merely because the projection is convenient.

## Historical contract synchronization

The FORUM-20Q Groups and FORUM-20AT Channel contracts remain historical source records for their adapters. Their current metadata and verifiers are updated only to acknowledge the downstream FORUM-26B authoritative trust wrapper; their original membership ownership and positive-union behavior remain unchanged.

## Canonical plan debt

The canonical `crates/rustok-forum/docs/implementation-plan.md` is intentionally not replaced through the GitHub contents API. The file exceeds two thousand lines and complete replacement risks unrelated roadmap loss. A later safe repository-local edit must mark FORUM-26 `in_progress`, record FORUM-26A/B, advance the FORUM-20 trust dependency, and retain explainable evaluation, posting limits, duplicate hashing, shared rate limiting and optional scoring as remaining work.

`CRATE_API.md` is likewise not completely replaced in this slice. The public adapter is source-exported from the crate root and recorded by the machine contract and verifier.

## Validation status

The following commands are source-ready but were not run by the implementation agent:

```text
cargo test -p rustok-forum --test user_trust_audience_facts_sqlite -- --nocapture
cargo test -p rustok-server --features mod-forum forum_audience_facts -- --nocapture
cargo test -p rustok-server --features mod-forum,mod-groups forum_audience_facts -- --nocapture
node scripts/verify/verify-forum-user-trust-audience-facts.mjs
node scripts/verify/verify-forum-audience-channel-facts-host-runtime.mjs
node scripts/verify/verify-forum-audience-group-facts-host-runtime.mjs
cargo xtask module validate forum
```

Tests, Cargo commands, formatting, verifier execution, workflows and CI remain the maintainer's responsibility for this slice.
