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
- `SetVisibility { state: Removed }` for `forum_post` is admitted only through the complete
  Forum `ReplyService::remove_in_tx` owner path. It validates the existing transition to
  `ReplyStatus::Deleted`, performs accepted-solution cleanup, soft-delete/tombstone capture,
  public/solution accounting and the canonical deleted status event/projection in the same
  fenced receipt transaction;
- `Unpublished` remains unsupported and is not approximated as `Rejected`;
- temporary lock and the remaining restriction effects fail closed until an exact owner
  semantic and, where required, expiry-safe Forum state exists.

Direct domain actions and moderation-driven actions converge on the same domain invariants
and owner primitives. Whether a direct action also opens a case is host/product policy.

## Application lifecycle

Durable decision application must use receipt-first replay and explicit states such as
pending, applying, retryable, applied, rejected, and operator-review.

Required semantics:

- identity is tenant + decision ID + decision hash + subject;
- identical completed application replays before subject reads;
- the same decision ID with another hash conflicts;
- domain mutation and domain receipt/audit commit atomically;
- moderation records applied evidence only after the adapter returns a matching
  `ModerationDecisionApplication`;
- timeout, missing provider, and owner outage remain retryable;
- validation, unsupported effect, and stale revision never become success;
- crash recovery cannot double-apply a decision.

The Forum source slice demonstrates receipt-first domain application using the shared
Outbox owner-operation ledger. `PortContext.idempotency_key` must equal the decision UUID;
receipt admission binds the full immutable command before subject reads. Application then
fences the active Forum subject and dedicated moderation revision row. Success completes the
shared receipt in the same Forum transaction as the local effect and returns the
post-application Forum moderation subject revision. For reply `Hidden`, the same transaction
also contains the exact Forum status mutation, any required public-counter/stat updates,
`ForumReplyStatusChanged`, and category projection invalidation when those counters changed.
For reply `Removed`, the same transaction uses the shared Forum reply-removal owner helper so
soft-delete/tombstone capture, accepted-solution cleanup, public/solution accounting,
`ForumReplyStatusChanged`, category projection when required, moderation revision advancement
and the completed receipt commit or roll back together. Completed receipt replay occurs before
subject reads; a new attempt against an already soft-deleted reply is unavailable rather than
re-applied. Non-retryable domain failures may become terminal receipt errors, while retryable
storage/serialization failures leave the processing lease reclaimable. This is producer-side
source only; the Moderation owner still needs its durable application attempt state,
scheduler/backoff and host runtime materialization.

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
  trusted caller gate, no-op decisions, permanent topic lock, exact reply `Hidden`, and exact
  reply `Removed` through the complete Forum removal owner path, guarded by
  `scripts/verify/verify-forum-moderation-subject-adapter.mjs`.

## Next priorities

1. Add durable Moderation-owner decision-application operations, attempt state, leases,
   retry/backoff, crash/lost-response recovery, and applied-evidence validation.
2. Materialize the registered adapter factories in host runtime and expose bounded operator
   recovery; prove missing/unavailable adapters stay retryable.
3. Extend Forum only with another exact owner effect mapping. `Unpublished` may be considered
   only after its Forum lifecycle meaning is explicit. Add explicit expiry-safe state before
   temporary restrictions.
4. Retain PostgreSQL/SQLite migration/backfill/trigger evidence for Forum moderation revision
   clocks plus concurrent content/lifecycle edit versus permanent-lock/reply-hide/reply-remove
   application evidence, approved-to-hidden accounting/event atomicity and removed-reply
   tombstone/accepted-solution/accounting/event atomicity.
5. Add PostgreSQL Moderation active-case/decision-effect/revision-CAS evidence.
6. Add moderation-specific RBAC resources and tenant permission registration.
7. Publish transactional outbox contracts for report, case, decision, application, and
   appeal lifecycle events.
8. Integrate Groups as the membership-scoped expiry reference adapter, then Blog, Comments,
   Pages, Reviews, Marketplace, Media, Messaging, and Profiles.
9. Add versioned policies, premoderation, automated assessment providers, appeals, and
   capability-scoped account sanctions.
10. Publish admin queue/case/application surfaces only after owner runtime composition.

## Invariants

- no cross-domain foreign keys;
- every decision references the exact reviewed subject revision;
- decision effect is immutable, typed, versioned, and part of decision hash identity;
- moderation never writes domain-owned tables;
- domain modules never import moderation entities or services;
- domain-specific subject revision clocks remain domain-owned and are not copied into
  Moderation persistence as a second mutable source of truth;
- receipts replay before provider or subject reads;
- idempotency keys and actor identity come only from `PortContext`;
- immutable decisions are not rewritten after application;
- domain owners validate subject, scope, revision, hash, and effect applicability;
- automated providers return assessments, never destructive actions;
- provider absence or timeout never becomes allow/applied;
- account sanctions are applied only by their capability owner.

## Degraded modes

- moderation unavailable: existing domain enforcement remains authoritative; no new
  sanction is inferred;
- adapter missing/unavailable: application remains pending/retryable;
- domain owner unavailable: moderation retains durable intent and does not mark applied;
- moderation disabled: authorized domain-local enforcement may continue when product policy
  permits it, while report/case/appeal features are unavailable;
- stale revision: conflict and explicit re-review/new decision;
- missing/corrupt domain subject revision state: invariant failure, never guessed success;
- unknown effect version, legacy `effect: None`, or unsupported effect: reject without
  domain mutation.

## Verification required before promotion

- `cargo check -p rustok-moderation-api` and `cargo check -p rustok-moderation`;
- `cargo test -p rustok-moderation-api` and `cargo test -p rustok-moderation`;
- `cargo check -p rustok-forum --all-targets` and
  `cargo test -p rustok-forum moderation_subject -- --nocapture`;
- `node scripts/verify/verify-moderation-api-boundary.mjs`;
- `node scripts/verify/verify-forum-moderation-subject-adapter.mjs`;
- clean/upgraded PostgreSQL and SQLite decision-effect migration evidence;
- Forum moderation subject revision migration/backfill and topic/reply content/lifecycle
  trigger-advance evidence on PostgreSQL and SQLite;
- duplicate/missing/mismatched adapter registry behavior;
- typed-effect serialization, bounds, version, compatibility, request-hash, and decision-hash
  evidence;
- historical decision `effect: None` read and non-dispatch evidence;
- PostgreSQL duplicate-report, active-case, and case-revision contention tests;
- decision application crash/retry/lost-response recovery;
- Forum shared-receipt replay/request conflict, trusted caller, stale revision, permanent-lock,
  reply-hide and reply-remove versus concurrent content/lifecycle edit evidence;
- approved-to-hidden topic/category/author counter adjustment, status-event/projection
  atomicity and already-hidden no-op/replay evidence;
- removed-reply delete-revision/tombstone capture, accepted-solution cleanup, public/solution
  accounting, status-event/projection atomicity, receipt replay and already-removed unavailable
  evidence on PostgreSQL and SQLite;
- unsupported `Unpublished` evidence;
- replay and changed-hash conflict across moderation and domain receipts;
- stale revision, unsupported effect, tenant/scope isolation, and owner adapter tests;
- composed runtime, RBAC, outbox, transport, disabled-module, accessibility, and no-fallback
  evidence.

No new execution evidence is claimed by the Forum adapter source slice. Maintainer-run
verification remains required before promotion.