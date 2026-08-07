# FORUM-19 Moderation subject adapter

Status: **bounded source-ready slice / maintainer execution pending**

## Scope

FORUM-19 makes Forum a producer-side consumer of the neutral `rustok-moderation-api` application port. It does not move reports, cases, queues, decisions, appeals, application orchestration or cross-domain audit into Forum.

Forum registers two `ModerationSubjectAdapterFactory` instances:

- `forum/forum_topic` for Forum topics;
- `forum/forum_post` for Forum replies.

The factories are module runtime extensions only. Materialization of the adapter registry and durable decision-application orchestration remain responsibilities of the Moderation owner/host and are not claimed by this slice.

## Trusted application boundary

`apply_moderation_decision` accepts only `PortActorKind::Service` or `PortActorKind::System` callers and requires full write semantics. A direct user caller is rejected before owner storage is read.

The `PortContext.idempotency_key` must equal the immutable Moderation `decision_id`. Forum reuses `rustok-outbox::idempotency` and the shared `owner_operation_receipts` ledger under `owner_slug = forum`; no Forum-specific application receipt table is added.

Receipt admission happens before Forum subject reads. The full `ApplyModerationDecisionCommand`, including the Moderation-owned `decision_hash`, is immutably bound by the shared receipt request digest. Successful replay therefore returns the stored `ModerationDecisionApplication` without re-reading a now-changed subject.

Non-retryable application errors may be retained as terminal shared receipt failures. Retryable database/serialization errors keep the processing lease reclaimable instead of freezing a temporary failure into the decision replay forever.

## Forum-owned moderation subject revision

The existing Forum content revision used by Reactions/current-revision transport is deliberately **not** reused for Moderation. It captures content/metadata/delete history but is not a complete clock for lifecycle or enforcement changes such as `is_locked`.

FORUM-19 therefore introduces two small Forum-owned current-state clocks:

```text
forum_topic_moderation_subject_revisions
forum_reply_moderation_subject_revisions
```

These tables are not Moderation cases, decisions, audit history or application queues. They contain only tenant-scoped Forum subject identity plus one positive monotonic revision.

Existing subjects are backfilled with a positive opaque revision. Database triggers initialize future subjects and advance the clock when the reviewed Forum subject changes:

- topic core/lifecycle/enforcement state including category, author, status, metadata, pin, lock and soft-delete state;
- topic translation insert/update/delete;
- reply core/lifecycle/structural state including parent topic/reply, author, status, position and soft-delete state;
- reply body insert/update/delete.

The moderation subject clock is intentionally distinct from:

- Reactions subject revision;
- `forum_topic_revisions` / `forum_reply_revisions` row IDs;
- `updated_at` timestamps;
- the global `forum_domain_events.sequence_no`.

This gives `ModerationSubjectRef.revision` one subject-local monotonic owner identity that covers the state this adapter is allowed to review and mutate, without coupling Moderation to Reactions or to a global event offset.

## Exact revision and concurrency boundary

After shared receipt admission, the adapter transaction fences both the exact active Forum subject and its moderation revision row. PostgreSQL uses `SERIALIZABLE` plus row locks. SQLite obtains its writer reservation through the dedicated revision row, avoiding a no-op update on the Forum subject and therefore avoiding unrelated topic/reply update triggers.

A decision applies only when the reviewed moderation subject revision exactly matches the current Forum clock. A stale decision fails before mutation and is never retargeted.

The permanent topic lock goes through `TopicService::set_locked_in_tx`. The database trigger must advance the moderation revision in the same transaction; the adapter returns that post-application revision as `ModerationDecisionApplication.applied_revision`. A true no-op keeps the reviewed revision unchanged. An unexpected missing/non-advancing clock is an invariant failure, not a guessed success.

The reply visibility slice applies only the neutral `SetVisibility { state: Hidden }` effect to `forum_post`. It uses Forum's existing `ReplyStatus::Hidden` lifecycle and the same owner primitives used by the established moderator path. An already-hidden reply is an exact no-op. Every other source status must have a valid Forum transition to `Hidden`; invalid lifecycle transitions fail closed.

When an approved reply becomes hidden, the same owner transaction decrements the topic public reply count, category public reply count and author Forum reply statistics. Non-approved-to-hidden transitions do not alter those public counters. The transaction writes the canonical `ForumReplyStatusChanged` root event; that event is already a Forum Search full-scope source. When public category counters change, the canonical category projection invalidation is also written. Status, counters/statistics, events, moderation-revision advancement and the completed shared receipt therefore commit or roll back together.

A service/system actor UUID is carried into the Forum event when the trusted port actor identity is a non-nil UUID. Non-UUID service identities remain representable by the Moderation-owned cross-domain audit while Forum's existing optional `moderator_id` field stays `None`.

## Effects in this bounded slice

Supported:

- `NoDomainMutation` for topic and reply decisions such as warning/no-violation outcomes;
- permanent topic `Lock { effective_until: None }`, using the existing `TopicService::set_locked_in_tx` owner mutation and canonical Forum Search projection invalidation;
- `SetVisibility { state: Hidden }` for `forum_post`, using exact `ReplyStatus::Hidden` lifecycle/accounting/event semantics.

Explicitly deferred:

- temporary topic lock, because Forum does not yet own expiry-safe moderation enforcement state;
- `SetVisibility { state: Unpublished }`, because it is not yet proven to be exactly equivalent to a Forum reply lifecycle state;
- `SetVisibility { state: Removed }`, because Forum removal requires the complete soft-delete/tombstone/solution/counter owner path and must not be approximated by setting a status;
- topic remove/unpublish effects;
- interaction restrictions, require-edit, publication rejection, suspension, escalation and account-sanction recommendation.

Unsupported effects fail closed. `Unpublished` is not silently mapped to `Rejected`, and `Removed` is not silently mapped to `Deleted`.

## Ownership preserved

Moderation remains authoritative for report intake, cases, queues, immutable decisions, appeals, retry/application orchestration and cross-domain audit. Forum retains only topic/reply lifecycle, the Forum-owned moderation subject revision, local enforcement state and its own events/projection invalidation.

`rustok-forum` depends on `rustok-moderation-api`; it does not depend on the `rustok-moderation` owner crate and does not read Moderation persistence.

This change is unrelated to Reactions ownership. It adds no reaction catalog, state, command, aggregate, transport or presentation code to Forum, and the moderation revision clock is not a second Reactions revision system.

## Maintainer verification handoff

Suggested checks, intentionally not run while preparing this slice:

```bash
node scripts/verify/verify-forum-moderation-subject-adapter.mjs
cargo test -p rustok-forum moderation_subject -- --nocapture
cargo check -p rustok-forum --all-targets
cargo xtask module validate forum
git diff --check
```

Future runtime evidence should additionally cover migration/backfill on PostgreSQL and SQLite, trigger advancement for topic/reply content and lifecycle changes, shared receipt replay/request conflict, stale reviewed revision, concurrent translation/body/lifecycle edit versus topic lock or reply hide, trusted-caller enforcement, PostgreSQL serialization/reclaim, approved-to-hidden counter/stat/event atomicity, already-hidden replay/no-op behavior, and disabled/unmaterialized Moderation profiles.

No tests, Cargo commands, Node verifiers, formatting, migrations, database scenarios, workflows or CI were executed while preparing this slice.
