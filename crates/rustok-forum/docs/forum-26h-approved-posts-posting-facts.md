# FORUM-26H approved-posts posting facts

Status: source-ready / unvalidated

## Delivered boundary

This slice publishes the Forum-owned `ForumApprovedPostsFactPort` for the
`ApprovedPosts` posting-policy fact.

The authoritative sources are the current `forum_topics` and `forum_replies`
rows owned by Forum. The adapter performs one aggregate statement for the exact
tenant and user and returns only the combined numeric count.

## Topic semantics

Forum topics do not have a moderation approval status. A newly created topic is
immediately `open`; `open`, `closed`, and `archived` describe lifecycle state,
not approval history.

Every retained, non-deleted topic authored by the exact user therefore
contributes one approved post. A soft-deleted topic is excluded regardless of
its final archived lifecycle state. Hard-deleted rows are absent and cannot
contribute.

## Reply semantics

A reply contributes only when all of the following remain true in the current
snapshot:

- the exact tenant and user own the reply;
- the reply status is `approved`;
- the reply is not soft-deleted;
- its exact tenant-scoped parent topic is not soft-deleted.

Pending, rejected, hidden, flagged, deleted-status, and soft-deleted replies are
excluded. The adapter does not reconstruct historical approval transitions from
revisions or events.

## Empty and degraded behavior

An empty exact-user contribution set is authoritative `0`, not an unavailable
fact. No separate user-existence lookup is added.

Storage execution failures return retryable `Unavailable`. A missing aggregate
row, invalid aggregate value, negative count, unsupported backend, or checked-sum
overflow is an invariant violation rather than a synthetic policy value.

PostgreSQL and SQLite use equivalent bounded query templates. Both return
separate topic and reply counts that are checked and added in Rust.

## Context and privacy

The adapter requires the exact user actor, exact tenant, and read/deadline
`PortContext` before storage access. It exposes only a `u64` count and does not
expose topic IDs, reply IDs, lifecycle timestamps, moderation metadata, bodies,
or revision history.

`forum_user_stats`, solution counts, trust history, and aggregate profile
counters are not imported or read.

## Performance boundary

The current Forum schema has tenant/category/status/topic-oriented indexes, but
this slice does not add dedicated exact-author count indexes or retained query-plan
evidence. Posting owner enforcement is still excluded, so this source-ready
adapter is not yet claimed as a production write-path latency dependency.

Before enforcement consumes `ApprovedPosts`, a separate bounded hardening slice
must add the appropriate PostgreSQL and SQLite author-count indexes, capture
query plans against representative tenant cardinality, and preserve the same
owner semantics. This debt is explicit rather than hidden behind
`forum_user_stats`.

## Host composition

The server posting-fact facade now registers four unique owner providers:

1. authoritative Forum trust;
2. authoritative server/users account age;
3. authoritative Forum topic reading activity;
4. authoritative Forum approved posts.

The published runtime extension remains `Arc<ForumPostingPolicyFactsComposer>`.
The existing audience-facts capability remains separate and unchanged. There is
no shared distributed rate-limit reservation or execution in this composition.

Rules requiring active flags, moderation history, reputation, usage windows, or
bump age remain explicit unavailable facts until their named owners are
delivered.

## Explicit exclusions

This slice adds no:

- posting-policy evaluation or precedence change;
- topic, reply, edit, or bump owner enforcement;
- policy configuration persistence or administration;
- topic or reply write;
- author-count index migration or query-plan evidence;
- shared distributed rate-limit reservation, commit, release, or counters;
- duplicate-content hashing or retained fingerprint;
- external or AI scoring call;
- trust-state write or automatic promotion/demotion;
- event, worker, migration, GraphQL, REST, OpenAPI, admin, or storefront surface.

## Canonical documentation debt

The canonical Forum implementation plan and `CRATE_API.md` were not replaced
through the GitHub contents API because both require complete-file replacement
and the implementation plan is large. This owner note and the machine contract
record the bounded FORUM-26H boundary without risking unrelated documentation
loss.

## Next bounded slice

The next bounded FORUM-26 slice should audit the authoritative moderation/report
owner for `ActiveFlags` and `RecentModerationActions`. Missing owner capabilities
must remain explicit unavailable facts; no moderation state may be inferred from
`forum_user_stats`, reply status totals, or local policy heuristics.

The author-count index and query-plan hardening must be completed before any
posting owner begins invoking the policy composer synchronously.

## Validation status

The following commands were not run by the implementation agent:

```text
cargo test -p rustok-forum posting_policy_approved_facts -- --nocapture
cargo test -p rustok-server --features mod-forum host_runtime_extensions_register_admin_mutation_providers -- --nocapture
node scripts/verify/verify-forum-approved-posts-posting-facts.mjs
node scripts/verify/verify-forum-approved-posts-index-debt.mjs
node scripts/verify/verify-forum-topic-reading-posting-facts.mjs
node scripts/verify/verify-forum-posting-policy-facts.mjs
cargo xtask module validate forum
```
