# Forum moderation lost-response PostgreSQL contract

Status: **source-ready / maintainer execution pending**

## Scope

`crates/rustok-distribution/tests/forum_moderation_lost_response_postgres.rs` is a real cross-owner PostgreSQL integration target for the response-loss window between Forum domain application and Moderation finalization.

The target is compiled only with both `rustok-distribution/mod-forum` and `rustok-distribution/mod-moderation`. Those dependencies already exist in the distribution feature graph, so this evidence adds no new crate dependency and no Cargo lockfile change.

It runs the production Outbox, Taxonomy, Forum and Moderation migrations in one isolated PostgreSQL schema. The Forum side uses the real `ReplyService`, real moderation subject revision triggers, real `ForumModerationSubjectAdapterFactory::reply()`, real `owner_operation_receipts`, real Forum counters/events and a real `Hidden` reply mutation. The Moderation side uses the real report/case/decision/application-operation owner and the ordinary one-attempt dispatcher.

## Lost-response scenario

The test creates one approved Forum reply and records its current Forum moderation subject revision. Moderation then creates an immutable `Hide + SetVisibility(Hidden)` decision against that exact revision.

The first producer call intentionally bypasses only the **Moderation finalizer**, not either owner:

1. the real Forum adapter receives the exact immutable Moderation application command with `PortContext.idempotency_key = decision UUID`;
2. Forum locks the active reply/revision, applies `Approved -> Hidden`, adjusts topic/category/user reply counts, emits the canonical `forum.reply.status_changed` event, advances the dedicated moderation revision and completes the Forum owner-operation receipt in the same serializable transaction;
3. the returned `ModerationDecisionApplication` is deliberately discarded as though the response was lost before Moderation could record it;
4. Moderation therefore still has a `pending` application operation and `decided` case even though Forum has already committed the effect.

The test then invokes the ordinary `ModerationService::dispatch_application_operation_once` with the same real Forum adapter registered under its exact key. At that point the immutable reviewed revision is intentionally stale in Forum because the first application advanced it. The second adapter call can succeed only if `owner_operation_receipts` returns the completed response **before** the adapter reaches its subject-revision fence.

## Assertions

A passing run proves all of the following on real PostgreSQL storage:

- the first Forum application advances the reply moderation revision and changes the reply to `hidden`;
- topic/category/user public reply counters move from one to zero exactly once;
- exactly one `forum.reply.status_changed` outbox event exists after the first application and still exactly one exists after the dispatcher retry;
- exactly one completed Forum owner-operation receipt exists under owner `forum`, operation `apply_moderation_decision`, and idempotency key equal to the immutable decision UUID;
- before retry, Moderation remains `pending`/`decided`, reproducing the cross-owner response-loss window rather than pre-finalizing the operation;
- the dispatcher retry ends `applied` and closes the Moderation case even though Forum's live subject revision is now newer than the immutable reviewed revision;
- Forum reply status, counters and moderation revision do not change on replay, proving the producer mutation was not executed twice.

This is the real producer receipt evidence intentionally left open by the earlier dispatcher test double. It verifies the exact receipt-first behavior required for retry after a lost response.

## Database isolation

The test reads `RUSTOK_FORUM_TEST_DATABASE_URL`, then `RUSTOK_MODERATION_TEST_DATABASE_URL`, then a PostgreSQL `DATABASE_URL`. Without a PostgreSQL URL it exits successfully with a skip message.

Each invocation creates `rustok_forum_moderation_lost_response_<uuid>`, sets `search_path`, creates only the minimal platform `users` prerequisite already used by Forum PostgreSQL tests, runs the production owner migrations, and drops the schema with `CASCADE` during cleanup.

## Maintainer commands

Intentionally not run while preparing this slice:

```bash
RUSTOK_FORUM_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-distribution \
  --features mod-forum,mod-moderation \
  --test forum_moderation_lost_response_postgres -- --nocapture

node scripts/verify/verify-forum-moderation-lost-response-postgres.mjs
```

No tests, Cargo commands, Node verifiers, formatting, real database migrations, workflows or CI were executed while preparing this file.
