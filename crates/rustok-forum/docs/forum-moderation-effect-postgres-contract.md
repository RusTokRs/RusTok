# Forum moderation effect PostgreSQL contract

Status: **source-ready / maintainer execution pending**

## Scope

`crates/rustok-forum/tests/moderation_effect_contract_postgres.rs` retains real PostgreSQL producer evidence for Forum reply moderation effects that were intentionally left distinct in FORUM-19:

- `RejectPublication` accounting, audit, idempotent replay, and already-rejected no-op behavior;
- `Remove + SetVisibility(Removed)` through the complete Forum reply-removal owner path, including accepted-solution cleanup and tombstone preservation;
- valid neutral `Unpublish + SetVisibility(Unpublished)` failing closed because Forum has no faithful unpublished lifecycle state.

The target uses the production Forum migrations through `PostgresForumTestDb` and materializes the real `ForumModerationSubjectAdapterFactory::reply()`. SQL is used only to seed truthful owner state and to observe the committed result; moderation mutations are performed only through the real adapter.

## RejectPublication

The fixture starts with one approved public reply and matching topic/category/author reply counters at one. Applying a real `RejectPublication` decision against the current Forum moderation revision must atomically:

- change the reply from `approved` to `rejected` without soft-deleting it;
- decrement topic, category and author public reply counters to zero exactly once;
- emit exactly one `forum.reply.status_changed` outbox event;
- advance the dedicated reply moderation revision;
- complete exactly one Forum owner-operation receipt under the immutable decision UUID.

Replaying the same decision UUID and request must return the stored application result without another state transition, event, counter adjustment or revision bump.

A second, fresh decision evaluated against the already-rejected **current** revision exercises the owner no-op path. Since `apply_reply_non_public_status_effect_in_tx` returns `changed=false` when the reply already has the target status, this decision completes truthfully at the same applied revision and does not emit another event or adjust accounting again.

## Removed accepted solution

The removal fixture starts with an approved reply that is also the topic's accepted solution. Public reply counters and the author's solution counter all begin at one.

`Remove + SetVisibility(Removed)` must route through `ReplyService::remove_in_tx`, the same owner path used for Forum reply deletion. A successful application must atomically leave:

- the reply row preserved as a tombstone with `status=deleted` and non-null `deleted_at`;
- the reply body preserved;
- the accepted-solution relation removed;
- topic/category/author reply counters at zero;
- author `solution_count` at zero;
- exactly one `forum.reply.status_changed` event to `deleted`;
- one completed Forum owner-operation receipt.

The same decision UUID is then replayed. The replay must return the exact stored `ModerationDecisionApplication` and leave the tombstone, counters, solution state, event count and moderation revision unchanged. This is a particularly strong receipt-first assertion: without completed receipt replay, normal active-subject lookup would reject the already soft-deleted reply.

## Unpublished remains distinct

The neutral Moderation API permits `ModerationDecisionKind::Unpublish` only with `SetVisibility(Unpublished)`, so the test supplies a structurally valid typed decision rather than a malformed command.

Forum deliberately does not approximate `Unpublished` as `Hidden`, `Rejected` or `Removed`. The real adapter must return non-retryable validation code `forum.moderation_effect_unsupported` and persist a failed owner-operation receipt. The reply must remain approved, its moderation revision and public counters must remain unchanged, and no status-change event may be emitted.

This keeps the shared semantic distinction intact: producers may support only the lifecycle states they can represent faithfully, and unsupported valid effects fail closed instead of being silently remapped.

## Relationship to other evidence

This target complements, rather than duplicates:

- `soft_delete_revision_postgres.rs`, which proves the generic Forum tombstone/revision owner invariant;
- `forum-lost-response-postgres-contract.md`, which proves lost-response replay across Moderation and Forum;
- `forum-moderation-revision-concurrency-contract.md`, which proves stale moderation cannot cross a concurrent content-edit revision fence.

Here the focus is the exact producer effect semantics and accounting/event/tombstone/solution consequences of successful or unsupported Forum moderation applications.

## Maintainer commands

Intentionally not run while preparing this slice:

```bash
RUSTOK_FORUM_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-forum --test moderation_effect_contract_postgres -- --nocapture

node scripts/verify/verify-forum-moderation-effect-postgres-contract.mjs
```

No tests, Cargo commands, Node verifiers, formatters, real PostgreSQL migrations, workflows or CI were executed while preparing this file.
