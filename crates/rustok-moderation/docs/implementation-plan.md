## Scope

Moderation ownership.

## Current state

Moderation module implementation.

## Milestones

1. Foundation

## Verification

- `cargo test -p rustok-moderation`

## Change rules

Standard rules.

---
id: doc://crates/rustok-moderation/docs/implementation-plan.md
kind: module_plan
language: en
status: in_progress
last_reviewed: 2026-08-07
---

# Moderation implementation plan

## Boundary

`rustok-moderation` owns reports, cases, policies, immutable decisions, durable
decision-application orchestration, appeals, moderation events, and cross-domain moderation
audit history.

Domain modules remain authoritative for their own subjects and enforcement state. A domain
owner validates and applies a moderation decision through a typed subject-owner port. The
moderation owner never writes domain-owned tables and never treats a queued or decided case
as proof that enforcement was applied.

## Neutral API

`rustok-moderation-api` is the neutral dependency shared by moderation and domain owners. It
contains no SeaORM entities, migrations, owner services, queues, or transports. It owns and
versions:

- `ModerationSubjectKind`, `ModerationScopeKind`, `ModerationScopeRef`, and
  `ModerationSubjectRef`;
- `ModerationReasonCode`, `ModerationDecisionKind`, and typed/versioned
  `ModerationDecisionEffect`;
- `ApplyModerationDecisionCommand` and `ModerationDecisionApplication`;
- `ModerationSubjectCommandPort`;
- the host-composed subject-adapter/factory registry keyed by
  `(subject_module, subject_kind)`.

`rustok-moderation` depends on the neutral API and temporarily re-exports moved contracts.
New domain adapters must depend only on `rustok-moderation-api`, never on moderation
persistence or services.

Registry rules:

- keys are normalized through a sealed constructor;
- registration is explicit and host-owned;
- duplicate adapter or factory keys fail startup;
- factories materialize only after `HostRuntimeContext` exists;
- a factory whose built adapter reports another key fails startup;
- a missing adapter leaves application pending/retryable and never implies success;
- no fallback adapter may apply a decision to another subject kind.

## Decision-effect compatibility

Decision kind alone is insufficient for temporary or capability-scoped sanctions. New
moderation decisions require a bounded typed effect with explicit schema version.

The v1 effect contract distinguishes:

- no domain mutation;
- hidden, unpublished, or removed visibility state;
- locking with optional expiry;
- interaction restriction with a bounded canonical capability set and optional expiry;
- edit requirement and publication rejection;
- subject suspension with `effective_until: Option<DateTime<Utc>>`;
- escalation and account-sanction recommendation, which unrelated owners do not apply.

The effect is validated against `ModerationDecisionKind`, included in command request
identity and immutable decision hash, and persisted in `moderation_decision_effects` in the
same owner transaction as the decision. Arbitrary owner payload JSON is not an enforcement
contract.

Historical decisions without an effect row remain readable as `effect: None`. They must not
be dispatched to a domain adapter without explicit re-review or a truthful migration; no
permanent sanction is inferred from an old decision kind.

## Subject identity and revisions

Every decision references the exact subject revision that was reviewed. Domain adapters
must expose a stable monotonic subject revision; timestamps and unrelated aggregate versions
are not substitutes.

For Groups compatibility:

- group subject: `module="groups"`, kind `Group`, ID `groups.id`, revision
  `groups.version`;
- membership subject: `module="groups"`, kind `GroupMembership`, ID
  `group_memberships.id`, revision a new monotonic `group_memberships.revision`;
- local scope: `ModerationScopeKind::Group` with `scope.id = group_id`;
- the Groups adapter verifies tenant, scope, subject ID/revision, decision hash, effect
  compatibility, and local invariants inside the owner transaction.

For Forum compatibility now present in source:

- topic subject: `module="forum"`, kind `ForumTopic`, ID `forum_topics.id`, revision from
  `forum_topic_moderation_subject_revisions.revision`;
- reply subject: `module="forum"`, kind `ForumPost`, ID `forum_replies.id`, revision from
  `forum_reply_moderation_subject_revisions.revision`;
- the two revision tables are Forum-owned current-state clocks only; they are not Moderation
  cases, decisions, audit history, queues, application-attempt state, or Reactions state;
- migration `m20260807_000027_add_forum_moderation_subject_revisions` backfills existing
  subjects and maintains future revisions with PostgreSQL/SQLite triggers over core
  subject/content/lifecycle/enforcement changes;
- the Forum adapter fences the exact non-deleted subject and its dedicated revision row before
  comparing the reviewed revision or applying a local effect;
- the Moderation revision is deliberately not the Reactions/content-revision row ID, an
  `updated_at` timestamp, or the global Forum event sequence;
- a stale revision conflicts and is never retargeted to current content/state.

A stale revision returns a stable conflict. Moderation requires explicit re-review/new
decision rather than silently retargeting the decision to the latest subject revision.

## Domain enforcement ownership

A domain may own current enforcement state because that state participates in access and
lifecycle invariants. It does not own the moderation case workflow.

For Groups:

- Groups owns effective membership suspension/ban state, expiry evaluation, membership
  revision, access denial, local receipts, domain audit, and semantic events;
- Moderation owns reports, cases, policy snapshots, decisions, application attempts,
  retries, appeals, and cross-domain history;
- Groups stores only bounded enforcement provenance required for replay and audit;
- Groups never copies reports, case notes, policy snapshots, appeal state, or queue data;
- moderation admin FFA owns queue/case/decision/application surfaces; Groups FFA owns current
  local enforcement state and authorized direct domain actions.

For Forum's bounded adapter source:

- Forum registers `forum_topic` and `forum_post` factories through the neutral API and has no
  dependency on the Moderation owner crate;
- Forum owns only its monotonic moderation subject revision clocks, existing topic/reply
  lifecycle/enforcement state, Forum counters/statistics, semantic events and Search
  projection invalidation;
- Forum reuses `rustok-outbox::idempotency` / `owner_operation_receipts` for bounded application
  provenance instead of creating a Forum case, decision, audit, or application-receipt
  subsystem;
- `NoDomainMutation` is accepted for both subject kinds;
- permanent topic lock maps to the existing Forum owner lock mutation plus Forum Search
  projection invalidation and advances the dedicated moderation subject revision;
- `SetVisibility { state: Hidden }` for `forum_post` maps exactly to `ReplyStatus::Hidden`;
  approved-to-hidden applies the existing topic/category/author public-count accounting,
  every changed hide publishes `ForumReplyStatusChanged`, and an already-hidden reply is a
  no-op without duplicate counters or events;
- `RejectPublication` for `forum_post` maps exactly to the established Forum moderator
  rejection lifecycle with target `ReplyStatus::Rejected`; approved-to-rejected applies the
  same public-count accounting, every changed rejection publishes `ForumReplyStatusChanged`,
  and an already-rejected reply is a no-op;
- neutral `SetVisibility { state: Unpublished }` remains a separate effect and is unsupported;
  Forum does not collapse it into `ReplyStatus::Rejected`;
- `SetVisibility { state: Removed }` for `forum_post` is admitted only through the complete
  Forum `ReplyService::remove_in_tx` owner path. It validates the existing transition to
  `ReplyStatus::Deleted`, performs accepted-solution cleanup, soft-delete/tombstone capture,
  public/solution accounting and the canonical deleted status event/projection in the same
  fenced receipt transaction;
- temporary lock and the remaining restriction effects fail closed until an exact owner
  semantic and, where required, expiry-safe Forum state exists.

Direct domain actions and moderation-driven actions converge on the same domain invariants
and owner primitives. Whether a direct action also opens a case is host/product policy.

## Host composition and shared work scheduling

The server host has source-ready optional materialization of the neutral subject-adapter
registry:

- `mod-moderation` selects the `rustok-moderation` owner independently from Forum;
- selecting that feature without `ModerationModule` in `ModuleRegistry` fails composition;
- factories materialize only after `HostRuntimeContext` exists and producer host facts are
  composed;
- Moderation without Forum materializes a valid empty registry;
- Forum with Moderation materializes `forum/forum_topic` and `forum/forum_post`;
- Forum without Moderation remains valid and does not materialize the owner registry;
- factory build, duplicate-key and factory-key mismatch errors remain startup failures.

Moderation reuses the platform's existing `rustok_runtime::ModuleWorkScheduler` instead of
creating a capability-specific polling loop. `ModerationModule::register_runtime_extensions`
publishes one `ModuleWorkRegistration` for `moderation_decision_application`. The existing
server module-work bootstrap registers it only in deployment modes that run background work,
uses the already-composed `HostRuntimeContext` and materialized adapter registry, polls through
the shared bounded scheduler, and honors the deployment-owned `StopHandle`.

The Moderation source returns at most one read-only earliest-due candidate per scheduler pass.
This source lookup is not the durable claim: `dispatch_application_operation_once` repeats
the owner due predicate and acquires the authoritative Moderation UUID lease through the
existing CAS before any adapter call. The generic `ModuleWorkItem.lease_token` is only
scheduler-envelope identity and never substitutes the Moderation lease or immutable decision
UUID domain idempotency key. Generic scheduler completion is a no-op because
`moderation_application_operations` remains the sole durable outcome source.

## Application lifecycle and owner audit

Durable decision application uses explicit owner states `pending`, `applying`, `retryable`,
`applied`, `rejected`, and `operator_review`.

Required semantics:

- identity is tenant + decision ID + decision hash + subject;
- identical completed domain application replays before subject reads;
- the same decision ID with another hash conflicts;
- domain mutation and domain receipt/audit commit atomically;
- moderation records applied evidence only after the adapter returns a matching
  `ModerationDecisionApplication`;
- timeout, missing provider, and owner outage remain retryable;
- validation, unsupported effect, and stale revision never become success;
- stale reviewed revision conflicts require explicit operator re-review/new decision;
- crash recovery cannot double-apply a decision.

The Moderation owner persists `moderation_application_operations` as one current operation
per immutable decision. `decide_case_replay_safe` creates the decision, typed effect, pending
operation, `case_decided` event and command receipt in one owner transaction. Migration
`m20260807_000004_create_moderation_application_operations` backfills only existing decisions
with a typed `moderation_decision_effects` row; historical `effect: None` decisions remain
non-dispatchable.

The operation snapshots the decision hash and exact reviewed subject. Bounded due reads and
CAS claim move due pending/retryable rows, or applying rows with expired leases, into
`applying`; every claim increments `attempt_count` and creates a fresh UUID lease token with
bounded expiry. Retryable completion sets an explicit bounded `next_attempt_at`. Applied,
rejected and operator-review completion require the exact unexpired lease token, so a stale
worker cannot finish after another worker reclaims an expired attempt.

`mark_application_applied` additionally requires returned `ModerationDecisionApplication`
evidence to match decision UUID plus exact subject module/kind/UUID/reviewed revision, and
requires `applied_revision >= reviewed_revision` before storing applied revision/time. The
persistence layer also rejects applying rows without a complete lease tuple and applied
revisions older than the reviewed revision.

`dispatch_application_operation_once` is source-ready as the bounded dispatcher. It claims
at most one exact due operation, reconstructs `ApplyModerationDecisionCommand` from immutable
decision/effect/case facts, verifies decision hash and exact reviewed subject, looks up only
the exact materialized `(subject_module, subject_kind)` adapter and invokes it with a trusted
service `PortContext`.

The domain call uses the immutable decision UUID as `PortContext.idempotency_key`; the current
lease token appears only in the attempt correlation ID. This is the lost-response boundary:
a retry after a successful domain mutation reaches the same domain receipt and replays before
subject reads instead of applying the mutation again. The port deadline budget is 30 seconds
while the dispatcher uses the existing default 60-second owner lease. If an adapter overruns
that budget and the lease expires, stale-token CAS prevents the old attempt from recording an
outcome after reclaim.

Missing exact adapters and retryable `PortError`s move to `retryable` with deterministic
bounded exponential backoff (5, 10, 20, 40, 80, 160 seconds, then capped at 300 seconds).
Non-retryable `Conflict` or `InvariantViolation` errors move to `operator_review`, including
Forum stale-reviewed-revision conflicts. Deterministic corruption discovered while rebuilding
the immutable command also moves to `operator_review`. Other non-retryable neutral port
errors move to `rejected`.

A successful adapter response is not itself proof of application. If the returned
`ModerationDecisionApplication` mismatches the immutable decision/reviewed subject, the live
attempt moves to `operator_review` under the bounded evidence-invalid outcome rather than
expiring and replaying deterministic bad evidence forever. Database or lease failures while
recording success are returned to the caller instead of being rewritten as operator/domain
outcomes. Moderation storage failure after claim likewise leaves the lease to expire/reclaim
rather than forging a domain result.

The shared module-work adapter adds scheduling only. It discovers one candidate and delegates
immediately to the one-attempt dispatcher. If two hosts discover the same row, only one can
win the authoritative CAS; the loser performs no domain call. A process failure after the CAS
leaves the operation `applying` until the existing lease expires and becomes discoverable
again. Shutdown stops future shared-scheduler claims while an already claimed operation may
finish its canonical dispatcher path.

Application operation state, case lifecycle and the existing `moderation_events` owner audit
ledger advance atomically inside Moderation owner primitives for claims/finalizers executed
after this source is active:

- the first winning application claim moves `decided -> applying_decision`, increments the
  case revision, and appends `case_application_started`; every winning claim appends
  `application_attempt_claimed`;
- retry/reclaim while the case is already `applying_decision` does not bump the case revision;
- retryable completion keeps the case `applying_decision` and commits the retry schedule plus
  `application_retry_scheduled` in the same owner transaction;
- accepted application evidence commits operation `applied`, case
  `applying_decision -> closed`, one case revision increment, `closed_at`, release of
  `active_deduplication_key`, `application_applied`, and `case_closed` together;
- application `rejected` or `operator_review` keeps those distinct operation outcomes but
  fails closed at case level through `applying_decision -> escalated`, one case revision
  increment, the corresponding application audit event, and `case_escalated` together;
- an escalated case retains its active deduplication identity for later operator recovery and
  report attachment; only a closed case releases it.

If the operation CAS, case CAS or audit insert fails, the transaction rolls back rather than
leaving a partially advanced case/application pair. `moderation_events` remains an internal
owner audit ledger; this source slice does not freeze a typed cross-domain Moderation event
family in `rustok-events`.

Upgrade compatibility is fail-honest. Application rows that were already terminal
(`applied`, `rejected`, or `operator_review`) before the atomic lifecycle source became active
are no longer due and therefore do not flow through its claim/finalizer path. The owner now
has an explicit replay-safe reconciliation command for those rows; it validates immutable
decision/case identity and stored terminal evidence, then aligns only the Moderation case.
It writes present-time reconciliation audit facts rather than fabricating historical lifecycle
events or timestamps, and it never invokes a domain adapter just to construct history.

## Operator recovery

`operator_requeue_application_replay_safe` is a human-user, receipt-backed owner command for
explicitly retrying the same immutable decision. It accepts only `rejected` or
`operator_review`; an already `applied` decision can never be requeued. The command binds a
positive expected case revision and bounded reason, validates exact decision/hash/subject/case
identity and terminal storage shape, then atomically moves the operation to `retryable` due
now and the case from `escalated` (or legacy pre-audit `decided`) to `applying_decision`.
`application_operator_requeued` and `case_application_requeued` retain the operator UUID,
reason, prior terminal status and prior error facts. The next shared-scheduler claim remains
the only path to the one-attempt dispatcher, so the domain idempotency key stays the immutable
decision UUID.

`operator_reconcile_legacy_application_replay_safe` aligns only already-terminal operation
truth with the Moderation case. Mapping is fixed: `applied -> closed` and
`rejected|operator_review -> escalated`. Applied reconciliation requires stored
`applied_revision >= reviewed_revision` plus stored `applied_at`; non-applied terminal rows
must not contain applied evidence and no terminal row may retain a lease tuple. A case already
in the matching terminal state returns a replay-safe no-op. A legacy `decided` or
`applying_decision` case is advanced with one case revision CAS. Closed reconciliation uses
the **current reconciliation time** for `closed_at` and releases the active deduplication key;
it does not pretend the case closed at the older domain `applied_at`. The only audit facts
written are `application_legacy_terminal_reconciled` and
`case_legacy_terminal_reconciled` at reconciliation time.

A true re-review is deliberately not an in-place recovery action. Reviewed subject revision
is part of immutable decision identity, so stale-review recovery must create a new case and a
new immutable decision from a freshly authorized producer-supplied subject revision. The old
escalated case/decision remains historical truth. Moderation does not fetch a producer's
current revision or silently rewrite/retarget the old decision.

The Forum source slice demonstrates the matching receipt-first domain side using the shared
Outbox owner-operation ledger. `PortContext.idempotency_key` equals the decision UUID; receipt
admission binds the full immutable command before subject reads. Application then fences the
active Forum subject and dedicated moderation revision row. Success completes the shared
receipt in the same Forum transaction as the local effect and returns the post-application
Forum moderation subject revision. Reply `Hidden` and `RejectPublication` share the same
bounded non-public lifecycle transaction; reply `Removed` uses the complete Forum removal
owner path.

The previously open FORUM-19 orchestration gaps are closed: dedicated `moderation_cases`
RBAC, the authorized recovery GraphQL transport and the explicit fresh-revision -> new case ->
new immutable decision re-review workflow are present. Repository-executable PostgreSQL/SQLite,
host-composition, dispatcher, scheduler, lost-response and concurrency evidence is also retained.
A module-owned admin UI remains broader Moderation product work; deployment promotion is deferred
to final production validation rather than keeping the bounded Forum integration open.

## Source completed

- owner crate, module metadata, schema, migrations, report/case/decision services, receipts,
  events, queue reads, revision CAS, and SQLite owner-contract coverage;
- active-case identity using `ON CONFLICT DO NOTHING` rather than continuing a failed
  PostgreSQL transaction;
- `rustok-moderation-api` with neutral subject/scope/reason/decision contracts;
- typed effect v1 with bounded canonical capability keys and kind/effect compatibility;
- host adapter/factory registries with duplicate and factory-key mismatch errors;
- temporary owner-crate re-exports for Rust source compatibility;
- `moderation_decision_effects` tenant-scoped persistence and migration dependency;
- new decision request/hash/event/record binding to the typed effect;
- truthful legacy decision reads using `effect: None`;
- source guard `scripts/verify/verify-moderation-api-boundary.mjs`;
- Forum as the first real domain adapter producer in source: `forum_topic`/`forum_post`
  factories, dedicated owner moderation-revision clocks, shared receipt/revision fencing,
  trusted caller gate, no-op decisions, permanent topic lock, exact reply `Hidden`, exact
  reply `RejectPublication -> ReplyStatus::Rejected`, and exact reply `Removed` through the
  complete Forum removal owner path, guarded by
  `scripts/verify/verify-forum-moderation-subject-adapter.mjs`;
- server host source materialization of the neutral adapter registry under explicit
  `mod-moderation` selection, with source profiles for Forum-disabled, Moderation-only,
  Forum+Moderation and selected-feature/missing-owner behavior, guarded by
  `scripts/verify/verify-moderation-host-composition.mjs`;
- durable application-operation foundation: typed-effect-only migration/backfill, atomic
  decision/effect/pending-operation enqueue, bounded due reads, UUID lease-token CAS claim,
  expired-lease reclaim, retryable/rejected/operator-review/applied transitions and exact
  applied-evidence validation, guarded by
  `scripts/verify/verify-moderation-application-operation.mjs`;
- bounded one-attempt application dispatcher: immutable command reconstruction, exact adapter
  selection, decision-UUID domain idempotency, bounded deadline/backoff, retry/review/rejected
  classification and applied-evidence handoff, guarded by
  `scripts/verify/verify-moderation-application-dispatch-once.mjs`;
- shared runtime application scheduling: one Moderation `ModuleWorkRegistration`, read-only
  earliest-due candidate discovery, canonical one-attempt CAS delegation, no-op generic
  completion and shared host stop/background-worker lifecycle, guarded by
  `scripts/verify/verify-moderation-application-work-scheduler.mjs`;
- atomic application/case audit lifecycle for newly executed claims/finalizers over existing
  owner storage: first-claim `decided -> applying_decision`, retry/reclaim without duplicate
  case revision, applied `-> closed` with active-key release, rejected/operator-review
  `-> escalated`, and matching internal `moderation_events` audit facts in the same owner
  transaction, guarded by `scripts/verify/verify-moderation-application-audit-lifecycle.mjs`;
- replay-safe operator application recovery: human-user same-decision requeue only for
  rejected/operator-review outcomes, explicit applied-requeue denial, expected case revision
  CAS, present-time legacy terminal reconciliation with no adapter invocation, and explicit
  fresh-case/new-decision re-review semantics, guarded by
  `scripts/verify/verify-moderation-application-operator-recovery.mjs`.

- dedicated Moderation recovery authorization through `moderation_cases:override` / effective
  `moderation_cases:manage`, with tenant permission vocabulary supplied by platform RBAC;
- host-owned authenticated GraphQL recovery transport for same-decision requeue, legacy-terminal
  reconciliation and fresh-revision re-review;
- replay-safe fresh-revision re-review that creates a new case and immutable decision without
  retargeting historical truth;
- retained repository evidence for PostgreSQL owner/recovery/application/dispatcher/scheduler and
  lost-response behavior, SQLite/PostgreSQL operation migration parity, host-composition failures,
  and Forum revision/concurrency/effect/accounting boundaries.

## Next priorities

1. Publish the remaining typed transactional/public Moderation application event contracts without turning the internal `moderation_events` audit ledger into an accidental cross-domain API.
2. Integrate Groups as the membership-scoped expiry reference adapter, then continue the accepted producer sequence (Blog, Comments, Pages, Reviews, Marketplace, Media, Messaging and Profiles) through `rustok-moderation-api` without cross-owner persistence reads.
3. Add versioned policies, premoderation, automated assessment providers, appeals and capability-scoped account sanctions while preserving immutable decision/effect identity and owner-side application evidence.
4. Build the broader module-owned Moderation admin queue/case/application UI over the existing authorized ports and transport. The recovery GraphQL boundary already exists; UI navigation is not a security boundary.
5. Keep Forum `SetVisibility(Unpublished)` and temporary/restriction effects fail-closed until Forum owns exact distinct lifecycle/expiry-safe semantics; do not approximate unsupported effects to existing statuses.

## Invariants

- no cross-domain foreign keys;
- every decision references the exact reviewed subject revision;
- decision effect is immutable, typed, versioned, and part of decision hash identity;
- every new typed decision commits one durable pending application operation atomically;
- historical decisions without typed effects remain non-dispatchable and are not backfilled;
- only a live UUID lease token may finish an applying operation; expired leases are reclaimable;
- the shared scheduler may discover a candidate but the existing Moderation CAS remains the
  sole durable operation claim before any domain adapter call;
- the generic module-work envelope token never substitutes the Moderation operation lease or
  immutable decision UUID domain idempotency key;
- generic module-work completion never writes a second Moderation applied/retry/rejected state;
- Moderation must not add a bespoke polling loop outside the shared `ModuleWorkScheduler`;
- the first winning operation claim advances a `decided` case to `applying_decision`; retries
  and expired-lease reclaim do not repeatedly advance the case revision;
- retryable application outcomes keep the case in `applying_decision`;
- only accepted matching application evidence closes a case, and operation `applied`,
  `closed_at`, active-deduplication release and matching owner audit facts commit together;
- rejected/operator-review outcomes never close the case; they escalate it with matching
  owner audit facts and preserve the active deduplication identity;
- application/case lifecycle transitions and `moderation_events` audit inserts are one owner
  transaction and cannot partially commit;
- the internal `moderation_events` audit ledger is not silently promoted into a public typed
  cross-domain event contract;
- operator recovery requires a human user actor, a command idempotency key, a bounded reason
  and exact expected case revision;
- same-decision operator requeue is allowed only from `rejected` or `operator_review`; an
  `applied` decision is never returned to retryable work;
- operator requeue preserves immutable decision UUID/hash/subject identity and reaches the
  domain only later through the existing scheduler + dispatcher path;
- terminal reconciliation never invokes a domain adapter and never invents historical
  application/case audit facts or timestamps;
- applied legacy reconciliation requires stored applied evidence and closes at reconciliation
  time; rejected/operator-review legacy reconciliation escalates and preserves active identity;
- a stale reviewed revision is re-reviewed only through a new case and new immutable decision
  built from a freshly authorized producer revision; an old decision is never retargeted;
- the dispatcher selects exactly the stored subject module/kind adapter and never falls back;
- every domain application attempt uses the immutable decision UUID as its idempotency key;
- retryable neutral errors and missing adapters never become applied or terminal rejection;
- non-retryable conflicts and invariant failures stop in operator review rather than being
  silently retried or collapsed into ordinary rejection;
- mismatched successful application evidence stops in operator review and never becomes
  `applied`;
- applied evidence must match the immutable decision and exact reviewed subject before the
  Moderation owner records `applied`;
- moderation never writes domain-owned tables;
- domain modules never import moderation entities or services;
- domain-specific subject revision clocks remain domain-owned and are not copied into
  Moderation persistence as a second mutable source of truth;
- receipts replay before provider or subject reads;
- immutable decisions are not rewritten after application;
- domain owners validate subject, scope, revision, hash, and effect applicability;
- automated providers return assessments, never destructive actions;
- provider absence or timeout never becomes allow/applied;
- account sanctions are applied only by their capability owner.

## Degraded modes

- moderation unavailable: existing domain enforcement remains authoritative; no new
  sanction is inferred;
- background workers disabled: durable application intent remains pending/retryable and no
  application is guessed or silently marked applied;
- duplicate scheduler candidate discovery across hosts: the existing Moderation CAS chooses
  at most one live attempt; losing hosts perform no domain mutation;
- shared runtime stop: no new module-work claims begin after stop while already claimed work
  may finish its canonical owner path;
- adapter missing/unavailable: one-attempt dispatch records retryable state, leaves the case
  `applying_decision`, and never marks applied;
- domain owner unavailable/timeout: retryable neutral errors schedule bounded backoff while
  the case stays `applying_decision`;
- owner storage/audit failure after claim or during finalization: the owner transaction does
  not partially advance operation/case/audit state; the live/expired operation lease remains
  the recovery boundary and no domain outcome is fabricated;
- worker crash while applying: the expired lease becomes reclaimable; the case remains
  `applying_decision` and stale lease tokens cannot complete the reclaimed operation;
- pre-audit terminal operation on upgrade: a human operator may run bounded reconciliation;
  terminal operation truth is preserved, no historical lifecycle is fabricated and no domain
  adapter is called merely to construct history;
- explicit same-decision recovery: rejected/operator-review may be requeued under a human
  receipt-backed case-revision CAS; applied remains terminal and cannot be requeued;
- stale reviewed revision/conflict: operation enters operator review and the case escalates;
  true re-review requires a new case/new decision at a freshly authorized producer revision,
  never retargeting the old decision;
- moderation disabled: authorized domain-local enforcement may continue when product policy
  permits it, while report/case/appeal features are unavailable;
- unsupported deterministic validation/not-found/forbidden outcome: rejected application and
  escalated case, never guessed success;
- missing/corrupt owner command identity or mismatched successful evidence: operator review
  and escalated case, never automatic replay to success;
- unknown effect version or legacy `effect: None`: no domain mutation.

## Verification required before promotion

- `cargo check -p rustok-moderation-api` and `cargo check -p rustok-moderation`;
- `cargo test -p rustok-moderation-api` and `cargo test -p rustok-moderation`;
- `cargo check -p rustok-forum --all-targets` and
  `cargo test -p rustok-forum moderation_subject -- --nocapture`;
- `node scripts/verify/verify-moderation-api-boundary.mjs`;
- `node scripts/verify/verify-forum-moderation-subject-adapter.mjs`;
- `node scripts/verify/verify-moderation-host-composition.mjs`;
- `node scripts/verify/verify-moderation-application-operation.mjs`;
- `node scripts/verify/verify-moderation-application-dispatch-once.mjs`;
- `node scripts/verify/verify-moderation-application-work-scheduler.mjs`;
- `node scripts/verify/verify-moderation-application-audit-lifecycle.mjs`;
- `node scripts/verify/verify-moderation-application-operator-recovery.mjs`;
- `cargo check -p rustok-server --no-default-features --features mod-moderation`;
- `cargo check -p rustok-server --no-default-features --features "mod-forum mod-moderation"`;
- `cargo test -p rustok-server --no-default-features --features mod-moderation --test moderation_composition_profiles`;
- `cargo test -p rustok-server --no-default-features --features "mod-forum mod-moderation" --test moderation_composition_profiles`;
- human-actor recovery enforcement, command receipt replay/changed-request conflict and
  expected case revision contention evidence;
- rejected/operator-review same-decision requeue, applied requeue denial, requeue from current
  escalated and legacy decided case states, next scheduler claim and immutable decision UUID
  domain idempotency evidence;
- applied/rejected/operator-review legacy-terminal reconciliation, already-consistent no-op,
  current reconciliation-time close semantics, active-key release/preservation, corrupt
  terminal evidence fail-closed behavior and proof of no domain adapter invocation;
- shared scheduler registration/background-worker-disabled/stop behavior, earliest-due
  selection and multi-host same-candidate CAS convergence evidence;
- first claim `decided -> applying_decision`, retry/reclaim without duplicate case revision,
  retry audit atomicity, applied + closed + active-key release + audit atomicity,
  rejected/operator-review + escalated + audit atomicity, audit-insert rollback, stale-token
  finalizer rollback and case revision CAS contention evidence;
- clean/upgraded PostgreSQL and SQLite decision-effect and application-operation migration
  evidence, including typed-effect-only backfill and legacy effectless exclusion;
- atomic decision/effect/pending-operation/event/receipt commit and replay evidence;
- due ordering/bounds, concurrent claim, lease expiry/reclaim, stale-token rejection,
  command reconstruction, exact registry selection, retry scheduling/classification,
  stale-conflict operator-review and applied-evidence mismatch operator-review evidence;
- lost-response evidence proving repeated domain calls keep the decision UUID idempotency key,
  replay the domain receipt rather than reapply, and close the Moderation case exactly once;
- owner-storage failure after claim followed by lease-expiry reclaim evidence;
- Forum moderation subject revision migration/backfill and topic/reply content/lifecycle
  trigger-advance evidence on PostgreSQL and SQLite;
- duplicate/missing/mismatched adapter registry behavior;
- typed-effect serialization, bounds, version, compatibility, request-hash, and decision-hash
  evidence;
- historical decision `effect: None` read and non-dispatch evidence;
- PostgreSQL duplicate-report, active-case, and case-revision contention tests;
- fresh-revision re-review/new-case/new-decision admin flow evidence once transport/UI exists;
- Forum shared-receipt replay/request conflict, trusted caller, stale revision, permanent-lock,
  reply-hide, reply-reject and reply-remove versus concurrent content/lifecycle edit evidence;
- approved-to-hidden and approved-to-rejected topic/category/author counter adjustment,
  status-event/projection atomicity and already-target no-op/replay evidence;
- removed-reply delete-revision/tombstone capture, accepted-solution cleanup, public/solution
  accounting, status-event/projection atomicity, receipt replay and already-removed unavailable
  evidence on PostgreSQL and SQLite;
- unsupported `SetVisibility(Unpublished)` evidence proving it is not collapsed into
  `RejectPublication`/`ReplyStatus::Rejected`;
- replay and changed-hash conflict across moderation and domain receipts;
- stale revision, unsupported effect, tenant/scope isolation, and owner adapter tests;
- composed runtime, RBAC, outbox, transport, disabled-module, accessibility, and no-fallback
  evidence.

No new execution evidence is claimed by the application operator-recovery source slice.
Maintainer-run verification remains required before promotion.

## Final production validation — DEFERRED

Deployment-dependent promotion is collected after the remaining Moderation source/backend product queue is exhausted. Live deployment selection, operator credentials, observed background-worker behavior and release provenance do not keep an otherwise complete implementation slice open.

Repository-executable engineering evidence is not deferred: SQLite/PostgreSQL migrations and owner contracts, concurrency/lease/replay tests, host composition profiles, source verifiers and isolated runtime scheduler tests remain part of the implementation boundary that changes them.
