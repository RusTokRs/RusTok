# FORUM-26A user trust state

Status: source-ready / unvalidated.

## Delivered

- `forum_user_trust_states` is the Forum-owned authoritative current trust state for one tenant/user and stores a bounded level `0..100` plus the current revision.
- `forum_user_trust_revisions` stores the immutable explanation history: previous and resulting level, typed change kind, bounded reason code and summary, actor snapshot, tenant-scoped idempotency key, and timestamp.
- Absence of a current row means trust level `0`; existing tenants and users require no backfill.
- `ForumUserTrustService::get`, `set`, and bounded `history` require `forum_topics:manage`. A managed change also requires an authenticated user actor.
- `set` currently publishes only `manual_override`. The stored enum reserves `policy_evaluation`, `reconciliation`, and `migration` for later owner workflows without claiming those workflows are implemented.
- Identical idempotency replay returns the original revision result without adding history. Reusing the same tenant key with a different user, level, actor, or explanation fails closed.
- PostgreSQL owner writes and insert triggers share the exact advisory-lock identity `{tenant_id}:{user_id}:trust` and salt `26`.
- PostgreSQL and SQLite require every revision to advance exactly once, require its previous level to match current state, and require the materialized state update to match the newly inserted immutable revision.
- Direct revision update/delete and current-state delete are rejected. Target trust rows use tenant/user composite ownership and restrict target deletion so audit history cannot disappear through a cascade.

## Ownership boundary

`forum_user_stats` remains an activity-counter projection containing topic, reply, and solution counts. `FORUM-26A` does not read it, copy it, backfill from it, or infer trust from it. Future policy evaluation may consume typed authoritative facts such as account age, reading, approved content, flags, reputation, and moderation history, but only through a separate owner policy slice.

The change actor is retained as an audit snapshot rather than a hard foreign key. This avoids making immutable Forum history depend on later user-row deletion mechanics; authenticated transport/owner composition remains responsible for supplying the actor identity.

## Excluded

- no trust facts adapter for `ForumAudienceFactsPort`;
- no topic-create, reply-create, visibility, or moderation enforcement change;
- no GraphQL, REST, OpenAPI, storefront, admin, or public transport DTO;
- no automatic posting-policy evaluator, trust promotion/demotion job, reputation formula, flag model, moderation-history model, rate-limit change, duplicate-content hashing, or external/AI scoring;
- no dependency or host/server source change.

The next bounded slice should publish a read-only trust facts adapter over `ForumUserTrustService` state. Automatic explainable evaluation should remain a later slice because several required facts are not yet authoritative Forum inputs.

## Canonical plan debt

The canonical `crates/rustok-forum/docs/implementation-plan.md` is intentionally not rewritten through the GitHub contents API. The file exceeds two thousand lines and complete replacement risks unrelated roadmap loss. A later safe repository-local edit must mark `FORUM-26` in progress, record `FORUM-26A`, advance the FORUM-20 trust dependency, and retain posting-policy evaluation, trust adapter composition, limits, duplicate hashing, rate limiting, and optional scoring as remaining work.

## Validation status

The following commands are source-ready but were not run by the implementation agent:

```text
cargo test -p rustok-forum --test user_trust_state_sqlite -- --nocapture
node scripts/verify/verify-forum-user-trust-state.mjs
cargo xtask module validate forum
```

Tests, Cargo commands, formatting, verifier execution, workflows, and CI remain the maintainer's responsibility for this slice.
