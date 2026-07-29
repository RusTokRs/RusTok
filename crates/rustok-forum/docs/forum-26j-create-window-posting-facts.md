# FORUM-26J create-window posting facts

Status: source-ready / unvalidated

## Delivered boundary

This slice publishes two Forum-owned posting-policy fact adapters:

- `ForumTopicCreatesWindowFactPort` for `TopicCreatesWindow` on `CreateTopic`;
- `ForumReplyCreatesWindowFactPort` for `ReplyCreatesWindow` on `CreateReply`.

Each adapter receives the exact tenant, user, action and configured observation
window already derived by `ForumPostingPolicyFactsComposer`. It performs one
bounded aggregate against the corresponding Forum owner table and returns one
`ForumPostingWindowCount` with the exact requested `window_seconds`.

## Authoritative semantics

The fact represents persisted owner create activity, not current publication
state. Every topic or reply row authored by the exact tenant/user and created
inside the inclusive `[observed_at - window, observed_at]` boundary contributes
once.

Rows remain counted after normal soft deletion. Reply moderation state also does
not reset the budget: approved, pending, rejected, hidden, flagged and
soft-deleted replies all represent a completed owner create operation. This
prevents ordinary moderation or user deletion from becoming a rate-limit reset.

A command that failed before creating an owner row contributes nothing. A
retention purge or exceptional hard delete removes the historical row and is not
reconstructed by this snapshot adapter. Production policy windows must therefore
remain shorter than the guaranteed retention horizon unless a later durable
usage ledger replaces the row snapshot.

## Context and failure behavior

Both adapters require:

- an exact human-user actor matching the requested user;
- an exact tenant match;
- read policy and deadline semantics;
- a positive exact observation window already validated by the shared contract.

Storage execution failure is retryable `Unavailable`. Missing aggregate output,
unsupported backends, invalid counts, timestamp-boundary overflow or counts
outside `u32` are invariant violations rather than synthetic zeroes.

An empty exact scope is authoritative count `0`.

## Performance boundary

PostgreSQL and SQLite add partial author-time indexes:

```text
idx_forum_topics_tenant_author_created_at
    (tenant_id, author_id, created_at DESC)
    WHERE author_id IS NOT NULL

idx_forum_replies_tenant_author_created_at
    (tenant_id, author_id, created_at DESC)
    WHERE author_id IS NOT NULL
```

The source-ready database proofs bind each exact owner query to its index with
`EXPLAIN`, and inspect the persisted index definition. No backfill or owner-row
rewrite is required.

The previous approved-post indexes remain separate because their predicates and
join shape serve current retained publication facts, while these indexes serve
all persisted create activity regardless of deletion or moderation status.

## Host composition

The server posting-fact facade now registers six unique owner providers:

1. Forum trust;
2. server/users account age;
3. Forum approved posts;
4. Forum topic creates in the exact window;
5. Forum reply creates in the exact window;
6. Forum topic reading activity.

The shared composer still only gathers facts. It does not invoke the evaluator
from a posting command.

## Concurrency and enforcement exclusion

This count is an authoritative pre-decision fact, but it is not a concurrency-
safe distributed reservation. Two simultaneous creates may observe the same
count before either commits. Owner enforcement must therefore reserve capacity
through the future shared rate-limit capability before relying on these facts as
a hard limit.

This slice adds no topic/reply write guard, reservation, commit, release,
idempotency receipt or retry-after calculation.

## Explicit exclusions

This slice adds no:

- active-flag or moderation-history owner model;
- reputation ledger or reputation fact;
- edit-window fact, because current immutable revisions do not identify the
  editing actor consistently;
- bump-age fact, because the current fact request does not carry an exact topic
  target;
- policy persistence or administration;
- posting owner enforcement;
- distributed rate-limit execution;
- duplicate-content hashing;
- external or AI scoring;
- automatic trust promotion/demotion;
- GraphQL, REST, OpenAPI, admin or storefront surface.

## Next bounded slices

`ActiveFlags` and `RecentModerationActions` remain blocked on the authoritative
report/moderation owner under `FORUM-19`; they must not be inferred from reply
status totals or local heuristics.

A separate contract slice should add exact target/actor identity required for
edit-window and bump-age facts before those adapters are implemented. Posting
owner enforcement must remain later than shared reservation semantics.

## Validation status

The following commands were not run by the implementation agent:

```text
cargo test -p rustok-forum posting_policy_create_window_facts -- --nocapture
cargo test -p rustok-forum --test create_window_facts_index_sqlite -- --nocapture
cargo test -p rustok-forum --test create_window_facts_index_postgres -- --nocapture --test-threads=1
cargo test -p rustok-server --features mod-forum host_runtime_extensions_register_admin_mutation_providers -- --nocapture
node scripts/verify/verify-forum-create-window-posting-facts.mjs
node scripts/verify/verify-forum-posting-policy-facts.mjs
cargo xtask module validate forum
```
