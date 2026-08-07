# FORUM-19 Moderation subject adapter

Status: **bounded source-ready slice / maintainer execution pending**

## Scope

This slice starts FORUM-19 by making Forum a producer-side consumer of the neutral `rustok-moderation-api` application port. It does not move reports, cases, queues, decisions, appeals, application orchestration or cross-domain audit into Forum.

Forum registers two `ModerationSubjectAdapterFactory` instances:

- `forum/forum_topic` for Forum topics;
- `forum/forum_post` for Forum replies.

The factories are module runtime extensions only. Materialization of the adapter registry and durable decision-application orchestration remain responsibilities of the Moderation owner/host and are not claimed by this slice.

## Trusted application boundary

`apply_moderation_decision` accepts only `PortActorKind::Service` or `PortActorKind::System` callers and requires full write semantics. A direct user caller is rejected before owner storage is read.

The `PortContext.idempotency_key` must equal the immutable Moderation `decision_id`. Forum reuses `rustok-outbox::idempotency` and the shared `owner_operation_receipts` ledger under `owner_slug = forum`; no Forum-specific receipt table is added.

Receipt admission happens before Forum subject reads. The full `ApplyModerationDecisionCommand`, including the Moderation-owned `decision_hash`, is immutably bound by the shared receipt request digest. Successful replay therefore returns the stored `ModerationDecisionApplication` without re-reading a now-changed subject.

Non-retryable application errors may be retained as terminal shared receipt failures. Retryable database/serialization errors keep the processing lease reclaimable instead of freezing a temporary failure into the decision replay forever.

## Exact revision boundary

The adapter never retargets a decision. After receipt admission it locks the exact non-deleted Forum subject in the owner transaction and derives the current content revision with the same established Forum rule used by other neutral integrations:

```text
latest captured Forum topic/reply revision id + 1
```

If that value differs from `ModerationSubjectRef.revision`, the application fails with a revision conflict before any Forum effect is applied.

PostgreSQL uses a serializable read-write owner transaction. The selected subject row is taken into the transaction write set before the revision lookup; concurrent owner edits remain a retry/concurrency evidence gate rather than being claimed from source review alone.

## Effects in this bounded slice

Supported:

- `NoDomainMutation` for topic and reply decisions such as warning/no-violation outcomes;
- permanent topic `Lock { effective_until: None }`, using the existing `TopicService::set_locked_in_tx` owner mutation and canonical Forum Search projection invalidation.

Explicitly deferred:

- temporary topic lock, because Forum does not yet own expiry-safe moderation enforcement state;
- reply hide/reject and topic remove/unpublish effects;
- interaction restrictions, require-edit, publication rejection, suspension, escalation and account-sanction recommendation.

Unsupported effects fail closed. They are not approximated by unrelated existing Forum statuses.

## Ownership preserved

Moderation remains authoritative for report intake, cases, queues, immutable decisions, appeals, retry/application orchestration and cross-domain audit. Forum retains only topic/reply lifecycle, exact current revision, local enforcement state and its own projection invalidation.

`rustok-forum` depends on `rustok-moderation-api`; it does not depend on the `rustok-moderation` owner crate and does not read Moderation persistence.

This change is unrelated to Reactions ownership. It adds no reaction catalog, state, command, aggregate, transport or presentation code to Forum.

## Maintainer verification handoff

Suggested checks, intentionally not run while preparing this slice:

```bash
node scripts/verify/verify-forum-moderation-subject-adapter.mjs
cargo test -p rustok-forum moderation_subject -- --nocapture
cargo check -p rustok-forum --all-targets
cargo xtask module validate forum
git diff --check
```

Future runtime evidence should additionally cover shared receipt replay/request conflict, stale reviewed revision, concurrent edit versus permanent lock, trusted-caller enforcement, PostgreSQL serialization/reclaim, and disabled/unmaterialized Moderation profiles.

No tests, Cargo commands, Node verifiers, formatting, migrations, database scenarios, workflows or CI were executed while preparing this slice.
