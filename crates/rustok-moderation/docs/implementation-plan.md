---
id: doc://crates/rustok-moderation/docs/implementation-plan.md
kind: module_plan
language: en
status: in_progress
last_reviewed: 2026-07-22
---

# Moderation implementation plan

## Boundary

`rustok-moderation` owns reports, cases, policies, immutable decisions, durable
decision-application orchestration, appeals, moderation events, and cross-domain
moderation audit history.

Domain modules remain authoritative for their own subjects and enforcement state. A
domain owner validates and applies a moderation decision through a typed subject-owner
port. The moderation owner never writes domain-owned tables and never treats a queued or
decided case as proof that enforcement was applied.

## Neutral API prerequisite

Cross-domain contracts must not live only in the persistence owner crate. Introduce
`rustok-moderation-api`, following the neutral-contract pattern used by
`rustok-notifications-api`.

The neutral crate owns and versions:

- `ModerationSubjectKind`, `ModerationScopeKind`, `ModerationScopeRef`, and
  `ModerationSubjectRef`;
- `ModerationReasonCode`, `ModerationDecisionKind`, and a typed/versioned
  `ModerationDecisionEffect`;
- `ApplyModerationDecisionCommand` and `ModerationDecisionApplication`;
- `ModerationSubjectCommandPort`;
- a host-composed subject-adapter registry keyed by `(subject_module, subject_kind)`;
- adapter/factory registration contracts that contain no SeaORM entities, migrations,
  owner services, or transport implementations.

`rustok-moderation` depends on `rustok-moderation-api`. Domain modules depend only on the
neutral API when they publish adapters. During migration, `rustok-moderation` may
re-export the moved types and traits for Rust source compatibility, but new domain
integrations must not depend on the moderation persistence owner.

Registry rules:

- registration is explicit and host-owned;
- duplicate `(module, kind)` registrations fail startup;
- a missing adapter leaves decision application pending/retryable and never implies
  success;
- adapter resolution occurs only after host runtime dependencies are available;
- no fallback adapter may apply a decision to another subject kind.

## Decision-effect compatibility

The current decision kind alone is insufficient for temporary or capability-scoped
sanctions. `ApplyModerationDecisionCommand` must carry a bounded typed effect whose
schema version is included in the decision hash.

At minimum the effect model must distinguish:

- no domain mutation;
- visibility/publication changes;
- locking or interaction restriction;
- subject suspension with `effective_until: Option<DateTime<Utc>>`;
- capability-scoped restrictions with a bounded canonical capability set;
- escalation or account-sanction recommendation, which is not directly applied by an
  unrelated domain owner.

Arbitrary owner payload JSON is not an enforcement contract. Domain-specific metadata
may be referenced from immutable moderation evidence, but the adapter receives only the
typed fields required to validate and apply the decision.

## Subject identity and revisions

Every decision references the exact subject revision that was reviewed. Domain adapters
must expose a stable, monotonic revision for their subject; timestamps and unrelated
aggregate versions are not substitutes.

For Groups compatibility:

- group-level subject: `module="groups"`, kind `Group`, ID `groups.id`, revision
  `groups.version`;
- membership-level subject: `module="groups"`, kind `GroupMembership`, ID
  `group_memberships.id`, revision a new monotonic `group_memberships.revision`;
- local scope: `ModerationScopeKind::Group` with `scope.id = group_id`;
- the Groups adapter verifies tenant, scope, subject ID, subject revision, decision hash,
  effect compatibility, and local invariants inside the owner transaction.

A stale subject revision returns a stable conflict. Moderation retains the application
operation and may require re-review; it must not silently retarget a decision to the
latest revision.

## Domain enforcement ownership

A domain module may own current enforcement state because that state participates in its
access and lifecycle invariants. It does not own the moderation case workflow.

For Groups, this means:

- Groups owns effective group-membership suspension/ban state, expiry evaluation,
  membership revision, access denial, local command receipts, and domain audit;
- Moderation owns reports, cases, policy snapshots, decisions, application attempts,
  retries, appeals, and cross-domain moderation history;
- Groups stores only bounded enforcement provenance needed for replay and audit, such as
  `decision_id`, `decision_hash`, reason code, source kind, actor, effective interval,
  and resulting membership revision;
- Groups does not copy reports, case notes, policy snapshots, appeal state, or moderation
  queue data;
- the moderation admin queue and case UI belong to the moderation module; Groups UI may
  expose current local enforcement state and authorized domain actions, but must not
  implement a second case system.

Direct domain actions and moderation-driven actions must converge on the same Groups
owner command/invariant path. Whether a direct group-local action also creates a
moderation case is host/product policy, not a second persistence implementation.

## Application lifecycle

Durable decision application must use receipt-first replay and explicit states such as
pending, applying, retryable, applied, rejected, and operator-review.

Required semantics:

- the moderation application identity is tenant + decision ID + decision hash + subject;
- identical completed application replays the recorded result before subject reads;
- the same decision ID with another hash conflicts;
- the domain mutation and domain receipt/audit commit atomically;
- the moderation owner records applied evidence only after the adapter returns a valid
  `ModerationDecisionApplication` matching decision and subject identity;
- timeout or provider absence remains retryable;
- validation/stale conflicts are not converted into success;
- crash recovery cannot double-apply a decision.

## Completed

- owner crate and module metadata;
- typed subject and scope references;
- owner-neutral command/read ports and the initial subject-owner decision application
  port, currently located in the owner crate pending neutral API extraction;
- module-owned schema for reports, cases, report links, immutable decisions, receipts,
  and events;
- workspace and composed migration registration with locked build and migration-plan
  evidence;
- repository-backed report, case, assignment, and decision services;
- receipt-first replay and deterministic request hashes derived from `PortContext`;
- active-case identity using `ON CONFLICT DO NOTHING` rather than recover-after-unique-
  error transactions;
- revision compare-and-set for mutable case transitions;
- tenant-scoped read and queue projections;
- SQLite contract coverage for replay, conflicts, deduplication, revisions, and isolation.

## Next priorities

1. Extract and version `rustok-moderation-api`, add the host subject-adapter registry,
   and retain temporary re-exports from `rustok-moderation`.
2. Extend decision application with typed/versioned effects, including expiry and bounded
   capability restrictions.
3. Add durable decision-application operations, receipt replay, crash/retry recovery, and
   applied-evidence validation.
4. Add PostgreSQL concurrent active-case and revision-CAS evidence.
5. Add moderation-specific RBAC resources and tenant permission registration.
6. Publish transactional outbox contracts for report, case, decision, application, and
   appeal lifecycle events.
7. Integrate Groups first as the reference membership-scoped adapter, then forum, blog,
   comments, pages, reviews, marketplace listings/sellers, media, messaging, and profiles
   through owner adapters.
8. Add versioned policies, premoderation, automated assessment providers, appeals, and
   capability-scoped account sanctions.
9. Publish admin queue and case surfaces only after the owner runtime and adapter registry
   are composed.

## Invariants

- no cross-domain foreign keys;
- every decision references the exact subject revision that was reviewed;
- the moderation owner never writes domain-owned tables;
- domain modules never import moderation entities or services;
- receipts replay before provider or subject reads;
- idempotency keys and actor identity are accepted only through `PortContext`;
- active-case deduplication never relies on continuing a PostgreSQL transaction after a
  unique violation;
- immutable decisions are not rewritten after application;
- domain owners validate whether a decision and typed effect are applicable to their
  subject;
- automated providers return assessments, never direct destructive actions;
- application-provider absence or timeout never becomes an implicit allow or applied
  result;
- account-level sanctions are applied only by the account/capability owner, not by Groups
  or another unrelated subject owner.

## Degraded modes

- moderation owner unavailable: existing domain enforcement remains authoritative;
  report/case/decision workflows are unavailable, but domain reads do not infer new
  sanctions;
- subject adapter unavailable: decision application remains pending/retryable;
- domain owner unavailable: moderation keeps durable intent and does not mark applied;
- moderation module disabled: domain-local authorized enforcement may continue when the
  domain product policy allows it, while reporting/case/appeal features are unavailable;
- stale subject revision: fail with conflict and require re-review or an explicit new
  decision;
- unknown effect version or unsupported effect: reject without mutating the domain.

## Verification required before promotion

- neutral API dependency guard proving domain modules do not depend on the moderation
  owner crate;
- duplicate/missing subject-adapter registry behavior;
- typed-effect serialization, bounds, version, and decision-hash evidence;
- PostgreSQL duplicate-report and active-case contention tests;
- concurrent case revision CAS tests;
- decision application crash/retry and lost-response recovery;
- replay and changed-hash conflict across moderation and domain receipts;
- stale subject revision and unsupported-effect behavior;
- cross-tenant and local-scope authorization evidence;
- Groups group/membership subject identity and exact-revision adapter tests;
- owner adapter contract tests for each integrated module;
- composed runtime, RBAC, outbox, transport, disabled-module, and no-fallback evidence.
