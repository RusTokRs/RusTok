# FORUM-26G topic reading posting facts

Status: source-ready / unvalidated

## Delivered boundary

This slice publishes the Forum-owned `ForumTopicReadPostingFactPort` for the
`TopicsRead` posting-policy fact.

The authoritative source is `forum_topic_read_states`, whose primary key is the
exact `(tenant_id, topic_id, user_id)` tuple. The adapter counts rows for the
exact tenant and user. One retained row therefore contributes exactly one read
topic identity.

The existing read-tracking owner remains responsible for creating and advancing
read high-water rows. The posting-policy adapter is read-only and does not call
`mark_topic_read`, `mark_category_read`, `mark_all_read`, or any persistence
helper.

## Semantics

`TopicsRead` is a lifetime reading-ledger count. The adapter does not join the
current topic visibility scope and does not reinterpret soft-delete state. Topic
hard deletion remains governed by the existing topic/read-state persistence
relationship.

An empty exact-user ledger is authoritative `0`. It is not represented as an
unavailable fact and is never derived from `forum_user_stats`.

Storage failure returns retryable `Unavailable`. Validation, tenant mismatch,
foreign actor, and call-policy failures propagate as typed errors rather than
being converted to reading activity.

## Context and privacy

The adapter requires the exact user actor, exact tenant, and read/deadline
`PortContext` before storage access. It exposes only a bounded numeric count; it
does not expose topic identities, read positions, revision high-water values, or
timestamps.

## Host composition

The server posting-fact facade now registers three unique owner providers:

1. authoritative Forum trust;
2. authoritative server/users account age;
3. authoritative Forum topic reading activity.

The published runtime extension remains `Arc<ForumPostingPolicyFactsComposer>`.
The existing audience-facts capability remains separate and unchanged.

Rules requiring approved posts, active flags, moderation history, reputation,
usage windows, or bump age remain explicit unavailable facts until their named
owners are delivered.

## Explicit exclusions

This slice adds no:

- posting-policy evaluation or precedence change;
- topic, reply, edit, or bump owner enforcement;
- policy configuration persistence or administration;
- distributed rate-limit reservation, commit, release, or counters;
- duplicate-content hashing or retained fingerprint;
- external or AI scoring call;
- trust-state write or automatic promotion/demotion;
- event, worker, migration, GraphQL, REST, OpenAPI, admin, or storefront surface.

## Canonical documentation debt

The canonical Forum implementation plan and `CRATE_API.md` were not replaced
through the GitHub contents API because both require complete-file replacement
and the implementation plan is large. This owner note and the machine contract
record the bounded FORUM-26G boundary without risking unrelated documentation
loss.

## Next bounded slice

The next bounded FORUM-26 slice should add the authoritative approved-post fact
from Forum-owned approved topic/reply persistence. It must not use
`forum_user_stats` as a shortcut and must remain separate from posting owner
enforcement and distributed rate-limit execution.

## Validation status

The following commands were not run by the implementation agent:

```text
cargo test -p rustok-forum posting_policy_reading_facts -- --nocapture
cargo test -p rustok-server --features mod-forum host_runtime_extensions_register_admin_mutation_providers -- --nocapture
node scripts/verify/verify-forum-topic-reading-posting-facts.mjs
node scripts/verify/verify-forum-account-age-posting-facts.mjs
node scripts/verify/verify-forum-posting-policy-facts.mjs
cargo xtask module validate forum
```
