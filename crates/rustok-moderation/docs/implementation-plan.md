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

A stale revision returns a stable conflict. Moderation may require re-review; it never
silently retargets a decision to the latest subject revision.

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

## Host composition

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

Host composition itself does not schedule work or add domain logic. Durable application
intent/lease state and the bounded one-attempt dispatcher now live in the Moderation owner.
A future host/runtime scheduler may call that owner primitive with the already-materialized
registry; it must not bypass owner due/lease semantics.

## Application lifecycle

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

`dispatch_application_operation_once` is now source-ready as the bounded dispatcher. It
claims at most one exact due operation, reconstructs `ApplyModerationDecisionCommand` from
immutable decision/effect/case facts, verifies decision hash and exact reviewed subject,
looks up only the exact materialized `(subject_module, subject_kind)` adapter and invokes it
with a trusted service `PortContext`.

The domain call uses the immutable decision UUID as `PortContext.idempotency_key`; the current
lease token appears only in the attempt correlation ID. This is the lost-response boundary:
a retry after a successful domain mutation reaches the same domain receipt and replays before
subject reads instead of applying the mutation again. Adapter deadline is 30 seconds while
the dispatcher uses the existing default 60-second owner lease. If an adapter overruns the
lease, stale-token CAS prevents the old attempt from recording an outcome after reclaim.

Missing exact adapters and retryable `PortError`s move to `retryable` with deterministic
bounded exponential backoff (5, 10, 20, 40, 80, 160 seconds, then capped at 300 seconds).
Non-retryable `InvariantViolation` and deterministic corruption discovered while rebuilding
the immutable command move to `operator_review`; other non-retryable neutral port errors move
to `rejected`. Moderation storage failure after claim is returned to the caller and leaves the
lease to expire/reclaim rather than forging a domain result.

The Forum source slice demonstrates the matching receipt-first domain side using the shared
Outbox owner-operation ledger. `PortContext.idempotency_key` equals the decision UUID; receipt
admission binds the full immutable command before subject reads. Application then fences the
active Forum subject and dedicated moderation revision row. Success completes the shared
receipt in the same Forum transaction as the local effect and returns the post-application
Forum moderation subject revision. Reply `Hidden` and `RejectPublication` share the same
bounded non-public lifecycle transaction; reply `Removed` uses the complete Forum removal
owner path.

The remaining orchestration gap is the scheduler and audit lifecycle: enumerate bounded due
work in the selected runtime, call one-attempt dispatch, recover process crashes through
lease reclaim, and advance case/application lifecycle events/operator recovery without
weakening owner/domain idempotency. No background polling is claimed by this slice.

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
  selection, decision-UUID domain idempotency, bounded deadline/backoff, retry/terminal error
  classification and applied-evidence handoff, guarded by
  `scripts/verify/verify-moderation-application-dispatch-once.mjs`.

## Next priorities

1. Add the runtime scheduler/runner over the bounded one-attempt dispatcher: bounded due
   enumeration, process-crash recovery through lease reclaim, lifecycle ownership and safe
   shutdown/startup behavior without duplicating adapter invocation logic.
2. Define and persist case/application audit lifecycle around dispatch outcomes, including
   `decided -> applying_decision -> closed/escalated` semantics and transactional application
   lifecycle events; add bounded operator retry/requeue/re-review recovery.
3. Retain clean/upgraded PostgreSQL/SQLite application-operation migration/backfill evidence,
   atomic decision enqueue, due bounds/order, concurrent claim, lease expiry/reclaim,
   stale-token rejection, command reconstruction, exact adapter selection, retry/error
   classification, lost-response replay and applied-evidence validation.
4. Retain executable host-composition evidence for selected-owner/missing-owner,
   Moderation-only empty materialization and Forum+Moderation topic/reply materialization;
   prove factory build failures remain fail-closed.
5. Keep Forum `SetVisibility(Unpublished)` blocked until Forum owns an explicit lifecycle
   meaning distinct from `RejectPublication`; add explicit expiry-safe state before temporary
   restrictions and admit no lossy approximation.
6. Retain PostgreSQL/SQLite migration/backfill/trigger evidence for Forum moderation revision
   clocks plus concurrent content/lifecycle edit versus permanent-lock/reply-hide/
   reply-reject/reply-remove application evidence, approved-to-hidden/approved-to-rejected
   accounting/event atomicity and removed-reply tombstone/accepted-solution/accounting/event
   atomicity.
7. Add PostgreSQL Moderation active-case/decision-effect/revision-CAS evidence.
8. Add moderation-specific RBAC resources and tenant permission registration.
9. Publish remaining transactional outbox contracts and integrate Groups as the
   membership-scoped expiry reference adapter, then Blog, Comments, Pages, Reviews,
   Marketplace, Media, Messaging, and Profiles.
10. Add versioned policies, premoderation, automated assessment providers, appeals, and
   capability-scoped account sanctions; publish admin queue/case/application surfaces only
   after owner runtime composition.

## Invariants

- no cross-domain foreign keys;
- every decision references the exact reviewed subject revision;
- decision effect is immutable, typed, versioned, and part of decision hash identity;
- every new typed decision commits one durable pending application operation atomically;
- historical decisions without typed effects remain non-dispatchable and are not backfilled;
- only a live UUID lease token may finish an applying operation; expired leases are reclaimable;
- the dispatcher selects exactly the stored subject module/kind adapter and never falls back;
- every domain application attempt uses the immutable decision UUID as its idempotency key;
- retryable neutral errors and missing adapters never become applied or terminal rejection;
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
- adapter missing/unavailable: one-attempt dispatch records retryable state and never marks
  applied;
- domain owner unavailable/timeout: retryable neutral errors schedule bounded backoff;
- owner storage failure after claim: the attempt remains applying until lease expiry and is
  then reclaimable; no domain outcome is fabricated;
- worker crash while applying: the expired lease becomes reclaimable; stale lease tokens
  cannot complete the reclaimed operation;
- moderation disabled: authorized domain-local enforcement may continue when product policy
  permits it, while report/case/appeal features are unavailable;
- stale revision/unsupported deterministic domain effect: terminal non-success classification,
  never retargeted or guessed applied;
- missing/corrupt owner command identity: operator review, never adapter dispatch;
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
- `cargo check -p rustok-server --no-default-features --features mod-moderation`;
- `cargo check -p rustok-server --no-default-features --features "mod-forum mod-moderation"`;
- `cargo test -p rustok-server --no-default-features --features mod-moderation --test moderation_composition_profiles`;
- `cargo test -p rustok-server --no-default-features --features "mod-forum mod-moderation" --test moderation_composition_profiles`;
- clean/upgraded PostgreSQL and SQLite decision-effect and application-operation migration
  evidence, including typed-effect-only backfill and legacy effectless exclusion;
- atomic decision/effect/pending-operation/event/receipt commit and replay evidence;
- due ordering/bounds, concurrent claim, lease expiry/reclaim, stale-token rejection,
  command reconstruction, exact registry selection, retry scheduling/classification and
  applied-evidence mismatch evidence;
- lost-response evidence proving repeated domain calls keep the decision UUID idempotency key
  and replay the domain receipt rather than reapply;
- owner-storage failure after claim followed by lease-expiry reclaim evidence;
- Forum moderation subject revision migration/backfill and topic/reply content/lifecycle
  trigger-advance evidence on PostgreSQL and SQLite;
- duplicate/missing/mismatched adapter registry behavior;
- typed-effect serialization, bounds, version, compatibility, request-hash, and decision-hash
  evidence;
- historical decision `effect: None` read and non-dispatch evidence;
- PostgreSQL duplicate-report, active-case, and case-revision contention tests;
- scheduler/process crash/retry recovery and future case/application audit lifecycle evidence;
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

No new execution evidence is claimed by the bounded one-attempt application-dispatch source
slice. Maintainer-run verification remains required before promotion.