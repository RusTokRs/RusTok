# FORUM-19 Moderation subject adapter

Status: **bounded implementation complete / production promotion deferred**

## Scope

FORUM-19 makes Forum a producer-side consumer of the neutral `rustok-moderation-api` application port. It does not move reports, cases, queues, decisions, appeals, application orchestration, operator recovery or cross-domain audit into Forum.

Forum registers two `ModerationSubjectAdapterFactory` instances:

- `forum/forum_topic` for Forum topics;
- `forum/forum_post` for Forum replies.

The factories remain producer-owned neutral runtime extensions. The selected server host materializes the shared Moderation subject-adapter registry only when the optional `mod-moderation` owner feature is selected. The Moderation owner has durable application-operation persistence/leases, a bounded one-attempt dispatcher, source-ready registration into the existing shared `rustok_runtime::ModuleWorkScheduler`, atomic application/case audit lifecycle and bounded replay-safe operator recovery. Authorized recovery GraphQL transport/RBAC and explicit fresh-revision re-review are now present. Repository-executable PostgreSQL/SQLite, concurrency, host-composition and scheduler evidence is retained; only deployment-dependent production promotion is deferred.

## Host materialization boundary

`rustok-server::build_shared_runtime_extensions_with_host_providers` materializes the neutral Moderation adapter registry after module runtime extensions and Forum host facts are composed.

The source contract is explicit:

- `mod-moderation` remains an optional server/distribution owner feature and is not implied by `mod-forum`;
- when `mod-moderation` is selected, `ModerationModule` must be present in the supplied `ModuleRegistry`; a selected feature with a missing owner fails host composition;
- Moderation without Forum materializes a valid empty subject registry;
- Forum with Moderation materializes exactly the registered `forum/forum_topic` and `forum/forum_post` adapters;
- Forum without Moderation remains valid and its neutral adapter factories stay unmaterialized;
- factory build, duplicate-key or key-mismatch failures remain startup errors and never fall back to another adapter.

The host uses `HostRuntimeContext` for factory materialization. It does not copy Forum subject logic into `rustok-moderation`, and it does not create a second adapter implementation in the server. The existing generic module-work bootstrap later passes the already-composed host context to Moderation's work registration; no Forum-specific scheduler wiring is added to the server.

## Moderation-owned durable application operation and one-attempt dispatch

`rustok-moderation` owns one source-ready `moderation_application_operations` row per typed immutable decision. This storage is not Forum state and is never copied into Forum.

A new decision, typed effect, pending application operation, `case_decided` event and Moderation command receipt commit or roll back in one owner transaction. Upgrade backfill creates pending operations only for historical decisions that already have a typed `moderation_decision_effects` row. Legacy `effect: None` decisions remain non-dispatchable.

The operation snapshots the immutable decision hash and exact reviewed subject module/kind/UUID/revision. Its bounded lifecycle is `pending -> applying -> retryable|applied|rejected|operator_review`. Claiming uses a fresh UUID lease token, bounded expiry and attempt counter; an expired applying lease is reclaimable. Finishing an attempt requires the exact live lease token.

`ModerationService::dispatch_application_operation_once` handles one exact due tenant/decision operation per call. It reconstructs `ApplyModerationDecisionCommand` from the immutable Moderation decision/effect/case plus operation subject/hash, looks up only the exact materialized `(subject_module, subject_kind)` adapter and invokes it with trusted service actor `rustok-moderation`.

The domain call keeps **decision UUID** as `PortContext.idempotency_key`. The lease token appears only in the per-attempt correlation ID. This is required for lost-response safety: when Forum already committed a local effect and its shared receipt but the Moderation worker lost the response, a later retry uses the same decision UUID and Forum replays that receipt before subject reads instead of mutating again.

The dispatcher carries a 30-second port deadline budget and uses the existing default 60-second operation lease. Missing exact adapter and retryable `PortError` become `retryable` with bounded backoff (5, 10, 20, 40, 80, 160 seconds, then 300-second cap). Non-retryable `Conflict` (including stale reviewed revision) or `InvariantViolation`, plus corrupt immutable command state, become `operator_review`; other non-retryable neutral port errors become `rejected`.

`mark_application_applied` still accepts only `ModerationDecisionApplication` evidence matching the stored decision and exact reviewed Forum subject identity, with `applied_revision >= reviewed_revision`. If an adapter returns `Ok(...)` with mismatched application evidence, the live attempt moves to `operator_review` under `moderation.application_evidence_invalid`. Database/lease errors while recording success remain owner errors/reclaim paths rather than being rewritten as operator or domain outcomes. If Moderation storage fails after claim, the dispatcher returns the owner error and the lease is allowed to expire/reclaim; it does not manufacture a Forum/domain result.

This orchestration state does not replace Forum's domain receipt and does not move any Forum lifecycle logic into Moderation.

## Shared application work scheduler

`ModerationModule::register_runtime_extensions` publishes one `rustok_runtime::ModuleWorkRegistration` for worker slug `moderation_decision_application`. This reuses the platform's existing module-work lifecycle and does not add a Moderation-owned `tokio::spawn`, interval loop or server-side Forum switch.

The Moderation work source performs one read-only earliest-due candidate lookup across `moderation_application_operations` per scheduler pass. It mirrors pending/retryable due time and expired-applying lease discovery only to avoid needless scheduler calls. Candidate discovery is **not** the durable claim.

The handler immediately delegates to `dispatch_application_operation_once`, which repeats the canonical due predicate and creates the real Moderation UUID lease through the existing CAS before any adapter call. The generic `ModuleWorkItem.lease_token` is scheduler-envelope identity only; it is never reused as the Moderation lease or the domain idempotency key.

This gives safe multi-host behavior without a second worker protocol. Two hosts may read the same due candidate; only one can win the existing Moderation CAS. The loser receives `None` and performs no domain mutation. If a worker fails after the Moderation claim, the durable operation remains `applying` until the existing lease expiry/reclaim path makes it eligible again.

Generic `ModuleWorkSource::complete` is intentionally a no-op for Moderation. Applied/retryable/rejected/operator-review state is persisted by Moderation owner primitives and remains the sole completion truth.

The server registers all `ModuleWorkRegistrations` into one shared scheduler only in runtime modes that run background workers. Its deployment `StopHandle` stops future claims while already claimed work may finish. A missing host-materialized Moderation adapter registry fails work registration rather than silently running an unusable worker.

## Moderation application audit lifecycle

The Moderation owner couples operation state, case state and the existing `moderation_events` audit ledger in the same owner transaction for claims/finalizers executed after this source is active. No new audit table, migration or public cross-domain event family is introduced.

The exact source-ready lifecycle is:

- the first winning application claim moves the case from `decided` to `applying_decision`, increments the case revision and appends `case_application_started`; every winning claim appends `application_attempt_claimed`;
- retry/reclaim while the case is already `applying_decision` does not bump the case revision again;
- a retryable application result stores the retry schedule and appends `application_retry_scheduled` while the case remains `applying_decision`;
- accepted application evidence moves the operation to `applied` and the case to `closed`, increments the case revision, sets `closed_at`, clears `active_deduplication_key`, and appends `application_applied` plus `case_closed` atomically;
- `rejected` and `operator_review` remain distinct application states but both fail closed at case level by moving `applying_decision -> escalated`, incrementing the case revision and appending the corresponding application event plus `case_escalated` atomically;
- an escalated case keeps its active deduplication identity for later operator recovery/report attachment; only a successfully closed case releases that active identity.

If the application CAS, case CAS or owner audit insert fails, the owner transaction rolls back. Moderation therefore never records a newly finalized terminal operation without the matching case/audit state, and it never records a newly finalized closed case without accepted application evidence.

A crash after a committed claim leaves the case in `applying_decision` and the operation in `applying` until the existing lease expires. Reclaim writes another attempt audit fact without another case revision. Lost-response retries still use the immutable decision UUID as the Forum domain idempotency key, so a previously committed Forum effect replays its shared receipt before Moderation closes the case.

## Moderation-owned operator recovery

Moderation now owns two replay-safe recovery commands over the same operation/case state. Forum owns none of their storage, receipts or audit facts.

`operator_requeue_application_replay_safe` requires a human user actor with UUID identity, a Moderation command idempotency key, a positive exact expected case revision and a bounded reason. It may requeue only `rejected` or `operator_review`; an `applied` decision is permanently excluded from same-decision requeue. The command validates the immutable decision/hash/subject/case relationship and terminal storage shape, then atomically moves the operation to `retryable` due now and the case from `escalated` (or legacy pre-audit `decided`) to `applying_decision`. It writes `application_operator_requeued` plus `case_application_requeued`. It does **not** call Forum. The next shared-scheduler claim remains the only path to the one-attempt dispatcher, and that later domain call keeps the same immutable decision UUID idempotency key.

`operator_reconcile_legacy_application_replay_safe` handles terminal rows created before atomic application/case audit lifecycle. Exact terminal truth maps `applied -> closed` and `rejected|operator_review -> escalated`. Applied reconciliation requires stored `applied_revision >= reviewed_revision` and stored `applied_at`; non-applied terminal rows must not contain applied evidence, and no terminal row may retain a lease tuple. A case already in the matching terminal state returns a no-op. A legacy `decided` or `applying_decision` case advances through exact revision CAS. For an applied row, `closed_at` is the **current reconciliation time**, not the historical domain `applied_at`; this is intentionally truthful rather than fabricated history. Only present-time `application_legacy_terminal_reconciled` and `case_legacy_terminal_reconciled` facts are written. No Forum/domain adapter is invoked.

A stale reviewed decision cannot be “fixed” by retargeting it. The reviewed revision is part of immutable decision identity. True re-review therefore requires a **new Moderation case and new immutable decision** created from a freshly authorized producer-supplied revision. The old escalated case/decision remains historical truth. An authorized admin transport/RBAC and that explicit fresh-revision re-review workflow remain future owner/product work.

`moderation_events` remains an internal owner audit ledger. This source does not freeze a typed `rustok-events` Moderation application/recovery event family.

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

## Exact hidden and rejected reply lifecycle effects

Neutral `SetVisibility { state: Hidden }` for `forum_post` maps exactly to Forum's existing `ReplyStatus::Hidden` lifecycle. Neutral `RejectPublication` maps exactly to the established Forum moderator rejection action, whose target is `ReplyStatus::Rejected`.

Both effects use one bounded non-public status helper that preserves the established Forum status-transition and public-accounting rules:

- an already-hidden Hidden decision and an already-rejected RejectPublication decision are exact no-ops;
- every changed application must pass the existing `ReplyStatus` transition validator;
- when the source reply was `Approved`, the same owner transaction decrements the topic public reply count, category public reply count and author Forum reply statistics;
- transitions from another non-public state only change lifecycle state and do not alter those public counters;
- every changed application writes the canonical `ForumReplyStatusChanged` root event;
- when public category counters change, the canonical category projection invalidation is written in the same transaction.

`RejectPublication` and `SetVisibility { state: Unpublished }` are deliberately **not** synonyms. The neutral Moderation API versions them as distinct effects. Forum already has an exact moderator `reject_reply -> ReplyStatus::Rejected` contract, so `RejectPublication` can reuse it. Forum does not currently have a separate exact lifecycle meaning for neutral `Unpublished`, so `Unpublished` remains unsupported and must not be collapsed into `ReplyStatus::Rejected`.

Status, counters/statistics, events, moderation-revision advancement and the completed shared receipt commit or roll back together.

## Exact removed reply owner path

Neutral `SetVisibility { state: Removed }` for `forum_post` is supported only because the complete existing Forum removal workflow is reusable inside the adapter's already fenced owner transaction.

`ReplyService::remove_in_tx` is the single Forum-owned mutation path used by both direct reply deletion and Moderation removal. It does **not** merely set `ReplyStatus::Deleted`. It:

- claims the active reply and validates the existing Forum transition to `ReplyStatus::Deleted`;
- removes an accepted-solution relation when the removed reply owns it;
- performs the established `status = deleted` plus `deleted_at` soft delete, so the existing database guards capture delete revisions/tombstone history;
- decrements topic/category/author public reply accounting when the removed reply was approved;
- decrements author solution statistics when the removed reply was the accepted solution.

The helper returns only the exact owner facts needed for established event publication. The normal delete path keeps its existing authorization and `TransactionalEventBus` instance. The Moderation adapter calls the same mutation helper, then writes the same canonical `ForumReplyStatusChanged` fact with `new_status = deleted` through the transaction-only outbox helper and writes category projection invalidation only when public counters changed.

Because the adapter has already fenced the active subject/revision, a changed removal must advance the dedicated moderation revision on the `status/deleted_at` update. Completed receipt replay happens before subject reads, so a decision is not re-applied to the now soft-deleted subject. A new attempt against an already removed subject is unavailable rather than treated as another successful removal.

A service/system actor UUID is carried into Forum events when the trusted port actor identity is a non-nil UUID. Non-UUID service identities remain representable by the Moderation-owned cross-domain audit while Forum's existing optional `moderator_id` field stays `None`.

## Effects in this bounded slice

Supported:

- `NoDomainMutation` for topic and reply decisions such as warning/no-violation outcomes;
- permanent topic `Lock { effective_until: None }`, using the existing `TopicService::set_locked_in_tx` owner mutation and canonical Forum Search projection invalidation;
- `SetVisibility { state: Hidden }` for `forum_post`, using exact `ReplyStatus::Hidden` lifecycle/accounting/event semantics;
- `SetVisibility { state: Removed }` for `forum_post`, using the complete `ReplyService::remove_in_tx` soft-delete/tombstone/solution/counter owner path plus the canonical status event/projection;
- `RejectPublication` for `forum_post`, using the established Forum moderator `ReplyStatus::Rejected` lifecycle/accounting/event semantics.

Explicitly deferred:

- temporary topic lock, because Forum does not yet own expiry-safe moderation enforcement state;
- `SetVisibility { state: Unpublished }`, because the neutral API distinguishes it from RejectPublication and Forum does not yet own a separate exact unpublished lifecycle state;
- topic remove/unpublish effects;
- interaction restrictions, require-edit, suspension, escalation and account-sanction recommendation.

Unsupported effects fail closed. `Unpublished` is not silently mapped to `Rejected`. `Removed` is not a status-only approximation: any future removal path must continue to go through the complete Forum owner helper.

## Ownership preserved

Moderation remains authoritative for report intake, cases, queues, immutable decisions, appeals, retry/application orchestration, operator recovery and cross-domain audit. Forum retains only topic/reply lifecycle, the Forum-owned moderation subject revision, local enforcement state and its own events/projection invalidation.

`rustok-forum` depends on `rustok-moderation-api`; it does not depend on the `rustok-moderation` owner crate and does not read Moderation persistence. `rustok-server` may compose both selected modules and materialize the neutral registry; that host composition does not change producer ownership.

This change is unrelated to Reactions ownership. It adds no reaction catalog, state, command, aggregate, transport or presentation code to Forum, and the moderation revision clock is not a second Reactions revision system.

## Verification and production handoff

The bounded FORUM-19 implementation is complete in repository source. The follow-up slices that were previously listed as pending are already merged:

- dedicated `moderation_cases` RBAC plus the authorized recovery command port (#3202);
- host-owned authenticated GraphQL recovery transport (#3206);
- replay-safe fresh-revision -> new case -> new immutable decision re-review (#3209);
- PostgreSQL owner/recovery/application/dispatcher/lost-response/scheduler evidence (#3211, #3213, #3216, #3217, #3219, #3221);
- SQLite/PostgreSQL migration parity, executable host-composition failure coverage and Forum revision/concurrency/effect evidence (#3225, #3226, #3230, #3232, #3234).

The repository still intentionally does **not** claim a module-owned Moderation admin UI, a typed public application-lifecycle event family, support for every neutral Moderation effect, or production rollout/release readiness. `SetVisibility(Unpublished)` remains a distinct unsupported effect, and temporary/restriction effects still require exact expiry-safe Forum owner semantics before any future admission.

Deployment-dependent promotion is tracked only by `PROD-FORUM-19` in the Forum plan. Pending production validation does not reopen FORUM-19 implementation unless it exposes a genuine source/backend regression.
