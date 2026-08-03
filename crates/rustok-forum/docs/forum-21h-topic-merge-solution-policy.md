# FORUM-21H topic merge accepted-solution policy

## Status

`source_ready_maintainer_execution_pending`

FORUM-21H closes the accepted-solution subpolicy inside the existing bounded
FORUM-21B same-category topic merge owner. It does not add a second merge
command, receipt or semantic event.

Machine contract:

```text
crates/rustok-forum/contracts/forum-topic-merge-solution-policy.json
```

Cumulative merge contract:

```text
crates/rustok-forum/contracts/forum-topic-merge-owner.json
```

## Why this policy is explicit

`forum_solutions` identifies one accepted reply for one topic. Moving replies
between topics changes the composite solution relation even when the reply ID is
stable. The database also requires the solution's tenant, topic and reply to
match exactly.

The merge owner therefore cannot update reply ownership while a source solution
still points at the old topic, and it cannot silently choose between two
accepted replies without changing Q&A meaning.

## Outcome matrix

| Source | Retained target | Outcome |
| --- | --- | --- |
| no solution | no solution | merge without solution mutation |
| no solution | one valid solution | preserve the target row unchanged |
| one valid solution | no solution | transfer the source row to the target |
| one valid solution | one valid solution | fail with `FORUM_TOPIC_MERGE_SOLUTION_CONFLICT` |

A stored solution is valid only when its reply is non-deleted, approved and
owned by the exact tenant/topic relation recorded by the solution row.

## Source-only transfer

The transfer is part of the original merge transaction:

1. lock source and target topic rows;
2. acquire source and target solution scopes in deterministic UUID order;
3. read and validate source and target solution state;
4. retain source `reply_id`, nullable `marked_by_user_id` and `marked_at` in
   transaction memory;
5. delete the source solution row;
6. move the complete bounded source reply set to the retained target;
7. insert the solution row for the target with the retained marker fields;
8. re-read and validate the transferred relation;
9. continue the existing topic counters, source archival, semantic event,
   immutable receipt and projection invalidation writes;
10. commit once.

The intermediate deleted marker is never visible outside the transaction. Any
failure rolls back the delete, reply movement and every later merge mutation.

## Statistics

The transfer does not call `UserStatsService` and does not change
`forum_user_stats.solution_count`.

The accepted reply ID and reply author do not change. Only the parent topic
relation changes, so decrementing and incrementing the same author's solution
count would add failure surface without changing the canonical statistic.

## Competing solutions

When both source and target have accepted solutions, the merge returns:

```text
FORUM_TOPIC_MERGE_SOLUTION_CONFLICT
```

The error contains the attempted merge operation ID. It is detected after
solution locks and validity checks but before:

- solution deletion or insertion;
- reply movement or position changes;
- topic counter or lifecycle changes;
- `forum.topic.merged` publication;
- merge receipt insertion;
- Search projection invalidation.

No implicit target authority, newest-marker choice, author preference or reply
score heuristic is used. A future manager-selected resolution command may
choose a winner explicitly and audit that decision as a separate bounded slice.

## Shared solution mutation scope

Migration
`m20260803_000016_add_forum_topic_merge_solution_policy` installs a common
solution mutation boundary.

PostgreSQL:

- solution INSERT/UPDATE/DELETE locks affected topic rows in deterministic order;
- advisory lock seed `31` serializes the exact tenant/topic solution scope;
- merge takes the same scopes after its stronger source/target topic row locks.

SQLite:

- `forum_topic_solution_locks` records the touched tenant/topic scope;
- trigger writes participate in SQLite's database write transaction;
- all solution updates, including marker-only updates, touch the scope;
- merge explicitly touches both scopes before inspecting solution state.

The public moderation owner also acquires the shared topic/solution scope before
reading the current marker, accepted reply or author used for a statistics
delta. Mark, replacement and clear therefore compute their mutation from state
that cannot change underneath the transaction. Database triggers enforce the
same scope for direct writers that bypass the owner API.

## Database validity guard

PostgreSQL and SQLite reject solution INSERT and owner-key UPDATE unless:

- the topic exists in the exact tenant;
- the topic is non-deleted and not archived;
- the reply exists in the exact tenant and topic;
- the reply is non-deleted and approved.

DELETE remains permitted because clear-solution and the source-only transfer
must remove a valid row.

## Compatibility

FORUM-21H changes no public merge input or result, merge receipt schema,
`forum.topic.merged` payload, shared event catalog or projection target list.

The existing source-topic, target-topic and category invalidations already force
Search to rebuild solved-state projections after a successful merge. An exact
operation replay resolves the immutable merge receipt before current topic or
solution state and creates no additional solution mutation.

## Source-ready regression

`crates/rustok-forum/tests/topic_merge_sqlite.rs` covers:

- source-only transfer with exact marker-field preservation;
- owner read paths reporting the moved reply as the target solution;
- unchanged solution author statistics;
- exact replay preserving one transferred marker;
- target-only marker preservation;
- two-solution typed conflict with unchanged topics, replies, solutions,
  statistics, events, receipts and invalidations;
- cross-category rejection;
- direct pending-reply and archived-topic solution write rejection;
- cumulative merge atomicity, idempotency and append-only receipt behavior.

The focused source verifier additionally proves that moderation mark and clear
lock the solution scope before current-marker reads and statistics calculations.

## Remaining scope

FORUM-21 remains `planned`. FORUM-21A through FORUM-21H are bounded source-ready
partial slices. Remaining work includes:

- maintainer execution and retained SQLite/PostgreSQL evidence;
- explicit manager-selected resolution for competing solutions;
- public/admin merge transport composition;
- canonical aliases, redirects and route tombstones;
- cross-category merge;
- split, fork and reply-range workflows.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-merge-owner.mjs
node scripts/verify/verify-forum-topic-merge-solution-policy.mjs
cargo test -p rustok-forum --test topic_merge_sqlite -- --nocapture
```

No command above was run by the implementation agent, per maintainer request.
