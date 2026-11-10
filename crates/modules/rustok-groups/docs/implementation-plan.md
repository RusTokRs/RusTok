## Scope

Groups domain ownership.

---
id: doc://crates/modules/rustok-groups/docs/implementation-plan.md
kind: module_implementation_plan
language: en
status: active
owners:
  - rustok-groups
  - platform-community
last_reviewed: 2026-11-10
---

# `rustok-groups` canonical implementation plan

## Canonical-source policy

This file is the single source of truth for the Groups roadmap, implementation backlog,
FFA/FBA status, integration gates, and release evidence. Issues and pull requests are execution
records only. Every behavior change must update this plan in the same change. Source presence alone
never promotes a task to `done`.

## Status vocabulary

- `planned`: contract or implementation is not source-complete.
- `in_progress`: useful source exists, but runtime, parity, concurrency, security, accessibility,
  migration, or degraded-mode evidence remains open.
- `done`: implementation and every declared gate have executable evidence.
- `blocked`: another owner capability is required before safe work can continue.

## Architectural invariants

### Ownership and terminology

Groups owns group identity, localized presentation, group memberships, local roles, join policy,
invitations, membership applications, rules/questions, group-local enforcement, feature bindings,
command receipts, domain audit, and Groups semantic events.

A group membership is social participation in one group. It is not a paid subscription, commercial
membership plan, entitlement, organization seat, event attendance record, or chat participation.
Those concepts remain with their respective owners.

Groups does not own provider content, profiles, media binaries, notification delivery, search/feed
projections, moderation reports/cases/decisions, appeals, billing, subscriptions, or entitlements.
Cross-module composition uses typed IDs, neutral ports, semantic events, and host composition.

### Moderation compatibility

`rustok-moderation` owns reports, cases, immutable decisions, application orchestration, retries,
appeals, and cross-domain history. Groups remains authoritative for group and group-membership
mutations.

Compatibility uses the neutral contracts exported by `rustok-moderation`; Groups
does not access moderation entities, migrations, or owner services. The dependency
name is `rustok-moderation` in the current manifest, not a separate API crate.
Subject identity is fixed:

- group: module `groups`, kind `Group`, ID `groups.id`, revision `groups.version`;
- membership: module `groups`, kind `GroupMembership`, ID `group_memberships.id`, revision
  `group_memberships.revision`;
- scope: kind `Group`, ID `group_id`.

`groups.version` is not a membership revision. `updated_at` is not a revision contract. A stale
subject revision must conflict and must never be retargeted.

### Privacy and effective membership

- Public groups expose their public shell/features.
- Closed groups expose a summary shell; body, members, private features, and provider content require
  effective active membership or platform authority.
- Secret groups remain undisclosed to unauthorized users.
- Active suspension is not active membership for private content, posting, comments, invitation,
  application, management, governance, or provider ACL decisions.
- Expiry is evaluated with the Groups owner clock and never depends on cleanup.
- Corrupt or unsupported enforcement state fails closed.
- `groups.member_count` is a stored lifecycle active count, not an owner-clock effective-enforcement
  count. Temporary suspension/revocation never changes it; join/leave and other stored lifecycle
  transitions remain its mutation owners.

### Commands, replay, and locking

Writes require deadline and idempotency key. Tenant/key command admission occurs
before owner reads. Matching receipt replay precedes current aggregate/subject
lookup and locking, effective authorization, CAS, lifecycle validation, or mutation.
A missing or changed current subject cannot invalidate a committed receipt. A changed request using the same key
conflicts. Replay is never denied because membership authority changed after the original commit.

The canonical membership lock sequence is:

```text
Group -> GroupMembership -> GroupMembershipEnforcement
```

PostgreSQL/MySQL use row locks. SQLite obtains writer serialization through a no-op update of the
already resolved group before membership/enforcement reads. Authorization runs after receipt replay
and owner locking but before the first domain mutation. Commands that lock multiple memberships use
deterministic user UUID order, followed by deterministic membership-ID enforcement-row order.

All commands admit the tenant/key before reads. Application and invitation locators
resolve the owning group, then the group is locked before the child row is
re-read and locked. This prevents cycles when invitation acceptance or leave also
settles application state. Bulk review remains one transaction, audit, receipt,
and result per item.

## Current state

Source exists for:

- module manifest, migrations, RBAC, registration, admin/storefront packages, and generated host
  composition;
- tenant groups, translations, memberships, roles, feature bindings, receipts, audit, join/leave,
  delegation, and ownership transfer;
- bounded token/targeted invitations, redemption, targeted source events, and optional Notifications
  integration;
- exact-locale membership-application policies, append-only history, CAS, immutable snapshots,
  lifecycle, focused review, and bounded partial-result bulk review;
- monotonic membership revision and bounded current enforcement projection;
- owner-clock effective resolver and `GroupMembershipEnforcementReadPort`;
- direct `GroupMembershipEnforcementCommandPort` suspend/revoke with expected-revision CAS,
  receipt-first replay, hierarchy/owner protection, shared owner mutation, audit/events and bounded
  direct-local provenance;
- neutral `groups/group_membership` Moderation subject adapter with trusted canonical group-scope
  propagation, decision-ID/hash producer receipt binding before subject reads, exact membership
  revision fencing and `SuspendSubject` reuse of the shared expiry-aware Groups owner mutation;
- direct GraphQL suspend/revoke mutations composed into the stable final Groups mutation root and
  routed only through `GroupMembershipEnforcementCommandPort`;
- stored lifecycle active member-count semantics that remain independent from temporary owner-clock
  suspension/expiry/revocation while every enforcement mutation still advances group version;
- append-only membership suspend/revoke semantic events beside targeted invitation events;
- effective core `GroupsService` for access, redaction, transaction-aware join/rejoin and leave,
  membership listing, enabled features, and transaction-aware feature settings using the canonical
  owner lock protocol;
- effective localization management reads plus transaction-aware translation upsert/delete using the
  same owner-clock manager semantics and `Group -> GroupMembership -> GroupMembershipEnforcement`
  write lock protocol;
- effective governance role/ownership commands with group-serialized actor-bound replay,
  deterministic membership/enforcement locks, owner-reference consistency and owner-clock authority;
- executable governance/enforcement PostgreSQL evidence source for replay, actor binding, concurrent
  role-versus-suspension serialization, revision fencing and platform owner recovery;
- executable governance/enforcement SQLite evidence source using a shared temporary database file and
  independent SeaORM pools for the same replay/race/recovery contract;
- sealed effective public invitation/application services with compatibility module paths;
- transaction-aware invitation/application writes using the group/membership/enforcement lock
  protocol;
- secret application non-disclosure, authorization-first status handling, receipt-first replay,
  stable effective error codes, CAS conflict mapping, and per-item bulk semantics.

Evidence still open:

- compilation and executed unit/integration tests;
- PostgreSQL and SQLite migration/runtime evidence, including the expanded append-only event ledger;
- lock behavior and concurrent enforcement-change tests;
- direct enforcement replay, expected-revision contention, hierarchy, owner-protection, expiry,
  revoke, lifecycle-count invariance, audit/event atomicity and security evidence;
- executed native/GraphQL parity and schema/error-mapping evidence for direct enforcement;
- executed join/rejoin suspension and enforcement-vs-join serialization evidence;
- executed leave-during-suspension, legacy-ban preservation and enforcement-vs-leave serialization
  evidence;
- executed feature-settings suspension/expiry and concurrent enforcement-vs-write evidence;
- executed localization suspension/expiry, native/GraphQL parity, and concurrent enforcement-vs-write
  evidence;
- maintainer execution of the PostgreSQL and SQLite governance/enforcement evidence sources plus
  remaining governance suspension/expiry, stress/deadlock and native/GraphQL parity evidence;
- native/GraphQL parity, CAS, lifecycle, bulk-review, retry, recovery, security, and accessibility
  evidence for the broader module;
- provider ACL integration and remote/degraded profiles;
- moderation adapter runtime/replay/concurrency evidence and remaining durable producer/provider integration evidence.

## Program ledger

| ID | Status | Scope | Remaining gate |
|---|---|---|---|
| GROUPS-00 | in_progress | ADR, ownership map, phpFox parity, FFA/FBA contracts | executable architecture review |
| GROUPS-01 | in_progress | module skeleton, manifest, RBAC, migrations, host composition | build/module validation |
| GROUPS-02 | in_progress | identity, localization, visibility, join policy, features, audit/events | lifecycle/runtime/concurrency |
| GROUPS-03 | in_progress | memberships, join/leave, roles, ownership transfer, direct enforcement | remaining enforcement integration |
| GROUPS-04 | in_progress | typed summary/membership/access/localization/invitation/application/governance/enforcement ports | consumer/fallback runtime matrix |
| GROUPS-05 | in_progress | GraphQL/native transports, invitation acceptance/delivery | parity and Notifications evidence |
| GROUPS-06 | in_progress | localized policy, CAS, lifecycle, focused/bulk review, FFA UX | profiles/events/parity/concurrency/accessibility |
| GROUPS-07 | in_progress | revision, enforcement read/direct command/GraphQL, neutral membership Moderation adapter, effective core/join/leave/feature/localization/governance access, transactional invitation/application authorization | provider cutover plus moderation/direct runtime/concurrency/replay/parity evidence |
| GROUPS-08 | planned | dynamic feature-provider registry and navigation | registry/degradation evidence |
| GROUPS-09 | planned | Forum group spaces and ACL inheritance | Forum integration evidence |
| GROUPS-10 | planned | Blog and Pages/Wiki group contexts | owner/privacy evidence |
| GROUPS-11 | planned | Marketplace seller context and listing composition | seller/checkout evidence |
| GROUPS-12 | planned | Media, Events, and Chat providers | provider lifecycle/degradation |
| GROUPS-13 | in_progress | notifications, search/SEO, neutral moderation compatibility, profiles/media | consumer runtime and adapter |
| GROUPS-14 | in_progress | storefront/admin UX and localization | enforcement UX/accessibility/parity |
| GROUPS-15 | planned | feed/wall aggregation | feed owner/ranking evidence |
| GROUPS-16 | planned | analytics and operator observability | privacy-safe metrics |
| GROUPS-17 | planned | import/export, retention, deletion, tenant lifecycle | compliance/recovery |
| GROUPS-18 | planned | remote adapter profile and degraded modes | fallback/recovery evidence |
| GROUPS-19 | in_progress | release verification matrix/evidence registry | all open evidence resolved |

## GROUPS-06 membership-application contract

Owner tables are current policies, exact-locale translations, append-only policy revisions, and one
current application per tenant/group/user.

Required invariants:

- exact host-resolved locale; no English or first-row fallback;
- manager authorization before sensitive application status disclosure;
- policy writes compare ID/revision/locale under owner locking;
- stale forms return `groups.application_policy_changed` without owner mutation;
- submitted snapshots preserve exact policy identity and rendered questions/rules;
- bulk review accepts 1..50 unique IDs, requires confirmation, preserves request order, and returns
  per-item partial results;
- native and GraphQL use the same owner ports and never fall back implicitly.

Remaining work includes profile-backed candidate summaries, lifecycle semantic events, richer
management UX, legacy API deprecation, and executed parity/replay/race/security/accessibility proof.

## GROUPS-07 enforcement and moderation compatibility

### Source-complete foundation

- `group_memberships.revision` starts at one and is monotonic.
- Role/lifecycle/invitation changes and enforcement mutations bump membership revision.
- `group_membership_enforcements` stores one bounded current row per membership.
- Effective state distinguishes missing, active, inactive, suspended, and legacy banned.
- Future, expired, or revoked enforcement falls back to stored lifecycle immediately.
- `groups.member_count` counts stored lifecycle-active memberships. Enforcement never changes the
  stored status, so temporary suspend/revoke leaves the count unchanged and avoids requiring cleanup
  to restore a time-driven counter at expiry.
- No Groups dependency on moderation owner persistence exists.

The database trigger bridge remains transitional for revision maintenance, but the owner write
architecture now has an explicit shared Groups enforcement mutation used by the direct command and
used by the neutral moderation adapter.

### Transactional invitation/application cutover

Source-complete public write paths now include:

- invitation create/revoke;
- token and targeted invitation acceptance;
- compatibility policy upsert and application submit;
- CAS policy upsert and application submit;
- candidate cancellation;
- manager reopen;
- focused review and every bulk-review item.

Each path:

1. validates stateless request/deadline/idempotency inputs;
2. starts the owner transaction;
3. acquires tenant/key command admission before opening any owner read snapshot;
4. returns matching receipt replay or changed-request conflict;
5. locates the aggregate and acquires group/membership/enforcement locks;
6. evaluates effective manager/candidate state using the Groups clock;
7. performs authorization-first lifecycle/CAS validation and mutation;
8. commits state, version/revision effects, audit, and receipt together.

Read-only list/policy/history surfaces continue to use the canonical read resolver. Secret candidate
surfaces preserve not-found semantics before membership-specific denial. Public facades retain only
stateless PortContext validation so deadline/actor/tenant error codes remain stable; no effective
state is evaluated outside the owner transaction for writes.

Runtime proof remains open; source completeness does not prove SQLite/PostgreSQL contention,
timeout, retry, deadlock, or lost-response behavior.

### Stable effective errors

Transactional effective authorization preserves:

- `groups.membership_suspended`;
- `groups.membership_banned`;
- `groups.manager_required`;
- `groups.membership_already_active`;
- `groups.application_policy_changed` for CAS mismatch.

Direct enforcement additionally exposes stable fail-closed errors:

- `groups.membership_enforcement_revision_conflict`;
- `groups.membership_enforcement_owner_protected`;
- `groups.membership_enforcement_self_target`;
- `groups.membership_enforcement_already_suspended`;
- `groups.membership_enforcement_not_active`;
- `groups.membership_enforcement_source_conflict`.

### Source-complete direct enforcement command

`GroupMembershipEnforcementCommandPort` now owns one direct suspend/revoke operation with:

- a real user actor and bounded idempotency key;
- expected membership revision CAS;
- owner/admin/moderator hierarchy plus platform moderate/manage authority and hard owner protection;
- receipt-first replay after tenant/key admission and before group lookup/locking, with actor/group/request binding;
- deterministic membership/enforcement lock ordering;
- canonical reason codes and optional owner-clock expiry;
- stored lifecycle status preserved as restoration state;
- direct-local provenance with no Moderation decision identity;
- shared crate-private suspension/revocation owner mutations used by the neutral adapter;
- trigger-owned membership revision advance plus explicit group-version advance;
- unchanged stored lifecycle member count on temporary suspend/revoke;
- atomic audit, exact append-only membership semantic event and command receipt;
- direct revoke restricted to active direct-local suspension, so local moderation cannot erase a
  moderation-decision enforcement row;
- original suspension actor/source preserved when revoked; revoker identity is immutable audit/event
  provenance.

Migration `m20260808_000009_extend_group_domain_events_for_membership_enforcement` widens the
append-only event ledger to exact invitation/membership event pairs on PostgreSQL and SQLite. SQLite
rebuild preserves historical sequence/event IDs and reinstalls targeted-invitation plus immutability
triggers. Downgrade fails while membership events exist instead of deleting append-only history.

No bulk enforcement command is introduced before single-command runtime evidence.

### Source-complete direct enforcement GraphQL transport

The stable module manifest entrypoint remains `graphql_application_cas::GroupsMutationRoot`. Its
existing `MergedObject` now includes `graphql_membership_enforcement::GroupsMembershipEnforcementMutation`
as one additive component while preserving the application/invitation/governance/localization chain.
The two new fields are:

- `suspendGroupMembership`;
- `revokeGroupMembershipSuspension`.

The transport requires an authenticated user in the same tenant, constructs a five-second write
`PortContext`, forwards every effective permission as a claim, forwards the caller's idempotency
key, and delegates only to `GroupMembershipEnforcementCommandPort`. It does not pre-authorize only
platform moderators, because local owner/admin/moderator hierarchy is an owner-domain decision.

GraphQL does not write Groups tables, recompute hierarchy, rewrite revisions, or introduce fallback.
Owner stable conflicts/validation map to bad-user-input, owner authorization failures map to
permission-denied, and unavailable/timeout/invariant outcomes stay non-successful. The response is
the owner result: membership/group/enforcement revisions, lifecycle member count, effective status,
expiry/revocation state and replay flag.

Runtime schema execution, error-code parity, replay/CAS and authorization parity remain evidence
gates; source composition is not promoted to `done`.

### Source-complete join/rejoin effective authorization

`GroupCommandPort::join_group` now treats re-entry authorization and membership lifecycle mutation as
one Groups owner transaction. The command reserves the group writer before reading mutable aggregate
state, confirms the group is active, and then calls the transaction-aware enforcement resolver so the
remaining membership/enforcement locks follow the canonical
`Group -> GroupMembership -> GroupMembershipEnforcement` order.

An active suspension now returns the stable `groups.membership_suspended` identity and a legacy ban
returns `groups.membership_banned` before invitation/join-policy lifecycle mutation. Stored `active`,
`pending`, `invited` or `left` status never overrides current owner-clock enforcement state.

If join/rejoin serializes first, its lifecycle mutation and revision effect are visible before a later
enforcement command continues; a suspension prepared against the old membership revision must
conflict. If suspension serializes first, the join transaction observes the suspension and writes no
membership lifecycle, member-count or group-version state. Existing visibility, join-policy,
invitation and active-member return semantics are otherwise unchanged.

Compilation plus SQLite/PostgreSQL join-versus-enforcement evidence remains open; source completeness
is not promoted to `done`.

### Source-complete leave effective lifecycle

`GroupCommandPort::leave_group` now leaves the sealed effective facade instead of delegating to the
status-only legacy implementation. The command reserves the group writer, resolves and locks the
actor membership/enforcement rows, and performs lifecycle mutation in one Groups transaction. This
prevents a concurrent suspension from racing a stale group-version/member-count snapshot.

Temporary suspension does not imprison the participant in the group: a non-owner may still leave
while suspended. The stored lifecycle row becomes `left`, the enforcement projection is preserved,
and expiry/revocation later falls back to stored `left` without cleanup. If the leaving membership
was lifecycle-active, `groups.member_count` and `groups.version` advance atomically with the leave.

Legacy banned membership is different because the stored `banned` lifecycle itself is enforcement.
Leave therefore returns stable `groups.membership_banned` and never rewrites `banned -> left`, even if
a temporary suspension row is concurrently effective. This prevents leave from erasing deny-reentry
state. Current owners still must transfer ownership before leaving, and already-left membership
remains idempotent.

If leave serializes first, its membership revision makes a suspension prepared against the old
revision stale. If suspension serializes first, leave may still commit afterward under the same
`Group -> GroupMembership -> GroupMembershipEnforcement` locks; the final enforcement row remains
preserved while stored lifecycle is `left`.

Compilation plus SQLite/PostgreSQL suspension/leave, legacy-ban preservation and lifecycle-count
runtime evidence remains open; source completeness is not promoted to `done`.

### Source-complete feature-settings effective authorization

`GroupCommandPort::set_group_feature` now treats feature bindings as transactionally serialized Groups
owner state instead of evaluating manager authority on one database snapshot and writing the feature on
a later snapshot. The command starts one owner transaction, reserves the group writer first, confirms
group existence, and then evaluates `GroupManagerCapability::ManageSettings` through the canonical
`Group -> GroupMembership -> GroupMembershipEnforcement` effective-manager guard before reading or
writing the feature binding.

A concurrently committed suspension therefore cannot land between authorization and feature mutation.
Suspended/banned local managers receive the stable `groups.membership_suspended` /
`groups.membership_banned` identities and inactive/insufficient roles receive `groups.manager_required`.
Platform `groups:manage` remains the explicit authority bypass, but it still participates in group
serialization before changing feature state.

Feature insert/update and the corresponding `groups.version` advance now commit in the same transaction.
The stored lifecycle `member_count` is unchanged. Runtime SQLite/PostgreSQL suspension/expiry and
feature-write contention evidence remains open; source completeness is not promoted to `done`.

### Source-complete localization effective authorization

Localization management no longer authorizes against raw stored membership status. Read-only
`list_group_translations` first preserves exact group existence semantics, then evaluates the shared
Groups owner-clock manager state. Translation `upsert` and `delete` first serialize the group row and
then evaluate `GroupManagerCapability::ManageSettings` through the canonical transaction-aware
`Group -> GroupMembership -> GroupMembershipEnforcement` lock protocol before the first translation
read or write.

This means an active owner/admin role is authoritative only while its effective membership is active.
An active suspension returns `groups.membership_suspended`; legacy banned state returns
`groups.membership_banned`; an inactive or insufficient role returns `groups.manager_required`.
Expiry/revocation takes effect immediately through the Groups owner clock without cleanup. Platform
`groups:manage` authority remains the explicit bypass, while write serialization still retains the
group owner row before mutation.

Exact-locale behavior, last-translation deletion denial, translation mutation semantics and
`groups.version` advancement are unchanged. Runtime PostgreSQL/SQLite contention and native/GraphQL
parity evidence remain open.

### Source-complete governance effective authorization

`GroupGovernanceCommandPort` now follows the same owner serialization and effective-membership
boundary as enforcement/localization. Both `change_group_role` and `transfer_group_ownership`
admit the command and check receipt replay before locating/locking the group.
Replay identity is bound to tenant + group + actor + command + request hash,
so a caller cannot reuse another actor's completed governance receipt. Matching replay returns before
current membership/enforcement authorization, preserving lost-response semantics when authority has
changed after the original commit.

After replay admission, governance locks every required membership in deterministic user UUID order,
then every corresponding enforcement row in deterministic membership-ID order. Local role changes
require an effective-active owner/admin actor and an effective-active non-owner target. Ownership
transfer requires an effective-active current owner for local authority and an effective-active new owner.
Suspended/banned local actors or targets fail with the existing effective errors instead of being
treated as active because their stored lifecycle row still says `active`.

Platform `groups:manage` remains an explicit recovery authority. It may transfer ownership away from
a suspended current owner as long as the stored current-owner membership is still lifecycle-active
and its role agrees with `groups.owner_user_id`; the replacement owner must still be effective-active.
Any owner-reference/owner-role disagreement fails closed before mutation.

Role/ownership writes, membership revision trigger effects, group-version advance, audit and command
receipt remain in one Groups transaction. The existing GraphQL governance transport still calls only
`GroupGovernanceCommandPort`; no transport fallback or second governance mutation path is introduced.
Compilation, PostgreSQL/SQLite contention, replay/lost-response, suspension/expiry, platform recovery,
governance concurrency and native/GraphQL parity evidence remain open.

### Executable governance/enforcement PostgreSQL evidence source

`apps/server/tests/groups_governance_enforcement_postgres.rs` now retains an ignored, schema-isolated
PostgreSQL evidence source over the production Groups migrations and production governance/enforcement
ports. It covers actor-bound lost-response replay after the actor becomes suspended, a concurrent
role-change versus suspension race using the same prepared membership revision, and platform ownership
recovery from a valid suspended current owner.

The concurrency contract accepts only the two outcomes implied by group serialization: governance
wins and makes the prepared suspension CAS stale, or suspension wins and the later governance command
observes `groups.membership_suspended`. Both commands succeeding is forbidden, and the raced membership
revision must advance by exactly one material change.

The platform recovery fixture writes the already-defined moderation-owned enforcement projection shape
directly because the neutral Moderation adapter is not part of this slice; the actual transfer still
uses `GroupGovernanceCommandPort` and the production owner-clock resolver. This fixture is not adapter
evidence and does not relax the adapter dependency/receipt gates.

Status is **maintainer execution pending**. The executable source does not populate runtime
`governance_concurrency` or other evidence fields until it is actually run on PostgreSQL. The handoff is
documented in `docs/governance-enforcement-postgres-contract.md` and guarded by
`scripts/verify/verify-groups-governance-enforcement-postgres.mjs`.

### Executable governance/enforcement SQLite evidence source

`apps/server/tests/groups_governance_enforcement_sqlite.rs` mirrors the PostgreSQL replay/race/recovery
packet against a real temporary SQLite file and independent SeaORM pools. It applies the same production
Groups migration list and invokes only the production governance/enforcement ports.

The SQLite concurrency contract exercises the owner writer reservation rather than row locks. The first
command to execute the no-op `groups.version` update owns the writer; the other command must observe the
committed material change before continuing. The accepted outcomes therefore remain identical to
PostgreSQL: stale suspension CAS after a role win, or `groups.membership_suspended` after a suspension
win, with exactly one membership revision advance.

Replay remains actor-bound and must occur before current effective authorization. Platform recovery
uses the same valid moderation-owned suspended-owner projection fixture and the same production ownership
transfer port. `sqlite::memory:` is intentionally forbidden because independent pools would otherwise
observe independent databases and provide false concurrency evidence.

Status is **maintainer execution pending**. SQLite concurrency/replay/recovery is not runtime evidence
until this test is actually executed. The handoff is documented in
`docs/governance-enforcement-sqlite-contract.md` and guarded by
`scripts/verify/verify-groups-governance-enforcement-sqlite.mjs`.

### Source-complete moderation adapter

Implemented bounded mapping:

- `GroupMembership` plus `SuspendSubject { effective_until }` maps to the shared Groups owner
  suspension mutation;
- identical decision ID/hash replays before current subject reads; changed hash conflicts;
- subject revision, tenant, group scope, effect version, hierarchy, owner invariants and immutable
  decision provenance are validated in the Groups transaction;
- the adapter uses `source_kind=moderation_decision` and stores only bounded decision UUID/hash plus
  trusted actor provenance, never case/report/queue/policy data;
- unsupported effects and account sanctions are rejected without mutation;
- moderation records applied evidence only after a matching adapter result.

The adapter consumes neutral contracts exported by `rustok-moderation` plus shared
`rustok-outbox` producer receipts; it does not access Moderation owner persistence. Moderation carries the already-validated immutable case scope as a versioned
trusted `PortContext` claim without changing the historical `ApplyModerationDecisionCommand` receipt
shape. Groups binds that exact scope together with the command in its own receipt, then reuses the
owner mutation above rather than introducing a second enforcement state path. Runtime/replay/race
proof remains an explicit GROUPS-07 evidence gate.

### GROUPS-07 definition of done

- no ownership leakage between Groups and moderation;
- monotonic membership identity/revision evidence;
- permanent, expiring, revoked, and restored enforcement across every owner path;
- hierarchy, owner protection, tenant isolation, replay, stale revision, lifecycle member-count, and
  concurrency evidence;
- native/GraphQL parity for state and direct actions;
- missing/timeout/retry/lost-response adapter behavior;
- moderation-disabled mode preserves existing Groups enforcement;
- PostgreSQL/SQLite migration, lock, compatibility, accessibility, and no-fallback evidence.

## Milestones

### Remaining implementation order

1. Add the neutral moderation subject adapter over the shared enforcement owner mutation.
2. Convert provider ACL consumers and remote/degraded profiles.
3. Execute and retain the PostgreSQL/SQLite governance-enforcement evidence sources, then produce
   remaining direct-enforcement, join/rejoin, leave, feature-settings, localization, governance and
   adapter runtime, parity, concurrency, security, migration and accessibility evidence.

## Degraded modes

- Groups access unavailable: deny private content.
- Corrupt enforcement row: invariant failure; never infer active.
- Expired/revoked enforcement: owner-clock fallback without cleanup.
- Active suspension: remove membership authority without hiding public content or changing the
  stored lifecycle member count.
- Legacy banned membership: deny re-entry and preserve the stored ban on leave.
- Exact-locale policy unavailable: form unavailable; never choose another locale.
- Policy CAS conflict: write no owner state and require reload.
- Notifications unavailable: Groups owner writes still commit.
- Moderation disabled: existing Groups enforcement remains effective; moderation-driven application
  is unavailable.
- Adapter unavailable: moderation must not mark a decision applied.
- Search/index unavailable: owner writes commit and projections catch up later.

## Verification matrix

Required before readiness promotion:

```bash
cargo xtask module validate groups
cargo check -p rustok-groups --features graphql
cargo check -p rustok-groups-admin --features ssr
cargo check -p rustok-groups-storefront --features ssr
cargo test -p rustok-groups
RUSTOK_GROUPS_TEST_POSTGRES_URL='postgres://...' cargo test -p rustok-server --features mod-groups --test groups_governance_enforcement_postgres -- --ignored --nocapture
cargo test -p rustok-server --features mod-groups --test groups_governance_enforcement_sqlite -- --nocapture
node scripts/verify/verify-groups-boundary.mjs
node scripts/verify/verify-groups-join-enforcement-authorization.mjs
node scripts/verify/verify-groups-leave-enforcement-authorization.mjs
node scripts/verify/verify-groups-feature-enforcement-authorization.mjs
node scripts/verify/verify-groups-localization-boundary.mjs
node scripts/verify/verify-groups-governance-effective-authorization.mjs
node scripts/verify/verify-groups-governance-enforcement-postgres.mjs
node scripts/verify/verify-groups-governance-enforcement-sqlite.mjs
node scripts/verify/verify-groups-invitations-boundary.mjs
node scripts/verify/verify-groups-membership-applications.mjs
node scripts/verify/verify-groups-application-policy-cas.mjs
node scripts/verify/verify-groups-application-lifecycle.mjs
node scripts/verify/verify-groups-application-bulk-review.mjs
node scripts/verify/verify-groups-membership-enforcement-read-path.mjs
node scripts/verify/verify-groups-membership-enforcement-command.mjs
node scripts/verify/verify-groups-moderation-subject-adapter.mjs
node scripts/verify/verify-groups-membership-enforcement-graphql.mjs
node scripts/verify/verify-groups-effective-membership-access.mjs
node scripts/verify/verify-groups-effective-membership-invitations-applications.mjs
npm run verify:i18n:ui
npm run verify:frontend:host-ffa-contract
```

No Cargo check, test, migration, Node verifier, browser, or CI command was executed for this source
slice. All affected runtime evidence remains `null`.


## Change rules

Follow AGENTS.md rules.

## Owner consistency remediation — 2026-11-10

Group membership is social participation, not a commercial subscription.
The owner now uses ordinary Rust modules rather than inline copies or source
stitching. `application_owner`, `invitation_owner`, and `targeted_invitation_owner`
are private implementation modules; public facade paths remain unchanged.

Implemented source changes:

- Core create/join/leave/feature and localization commands persist receipts and
  command audit atomically. Admission serializes an exact tenant/key before reads;
  receipt replay is checked before the precondition and effective authorization.
- PostgreSQL admission uses a transaction advisory lock; SQLite reserves its writer
  before opening a snapshot. Group locks precede application/invitation row locks.
- Request joins return `groups.application_required` without pending membership.
- Approved candidates can resubmit after leaving. Invitation acceptance completes
  a pending application; leaving cancels it and advances aggregate version even
  when the stored active member count does not change.
- Exact membership lookup requires self/service/management authority. Public
  rosters expose active lifecycle members only; management rosters remain broader.
- Catalog locale/search filtering uses a database subquery with stable ID ordering.

Existing transport boundaries remain mandatory:
`GroupApplicationPolicyManagementReadPort` carries the selected exact locale in a
typed request and returns an empty management view for a missing translation.
`GroupApplicationCasCommandPort` owns CAS saves/submissions; the final GraphQL root does not expose legacy unconditional policy/submission writes.
`GroupApplicationLifecycleReadPort` and `GroupApplicationLifecycleCommandPort`
remain shared native/GraphQL owner contracts.

Verification commands include `verify-groups-application-policy-locales.mjs` and
`cargo test -p rustok-groups --test command_lifecycle`. The new behavioral tests use
production migrations and public ports, including file-backed SQLite replay.
They are source-complete but NOT executed here: Cargo is unavailable.
PostgreSQL contention/replay, GraphQL execution, client/browser parity and recovery
remain required before integration. `membership_application_policy_cas` remains `null`,
`membership_application_policy_locale_management` remains `null`, and
`membership_application_lifecycle` remains `null` as runtime evidence gates.
No audit phase is marked complete by this scoped remediation.

Prior verification outcome before the privacy/finalization follow-up: 13 targeted Node source guards passed
(boundary, membership applications, policy CAS/locales, lifecycle, bulk review,
native no-bypass, both effective membership contracts, join, leave, feature and
governance authorization). All 117 module Rust files parsed without syntax errors;
changed Rust sources were formatted with WASM rustfmt and `git diff --check` passed.
A two-connection Python SQLite probe confirmed that the empty receipt UPDATE
reserves the writer and that the next admitted transaction sees the committed
receipt; this is primitive-level evidence, not execution of the Rust owner tests.
The remediation gate's diff invariants passed, but its architecture stage is
blocked by missing Cargo (`cargo metadata`). The nine new Rust behavioral tests,
crate compilation and PostgreSQL tests remain unexecuted. No runtime evidence
field or readiness phase has been promoted.


### Privacy and finalization follow-up

- Candidate application surfaces hide secret group lifecycle and join policy before
  business-state checks, including CAS submission. Missing groups are not-found;
  nil candidate group IDs are validation failures before database lookup.
- Shared SQLite writer reservation now uses an empty receipt UPDATE and covers the
  initial exact membership-subject locator, not only command admission. This also
  covers Moderation and exact-subject helpers without Groups core command receipts.
- Moderation no longer discards rollback or terminal producer receipt failures.
  Application/receipt completion failures share explicit rollback. Uncertain
  finalization is retryable and preserves both diagnostic causes.
- Five regression tests added for secret active/archived/suspended groups, cancellation
  of a historical application after the group became secret, missing/nil targets,
  independent-connection SQLite writer exclusion without receipt mutation,
  and finalization error classification. No tests or verifier suites were executed
  in this follow-up, per the maintainer's instruction to defer execution.

### Revision and counter bounds follow-up

- Shared private domain helpers reject non-positive stored revisions, signed
  revision overflow, negative member counts and member-count overflow.
- Core join/leave/features, translations, governance, applications and both
  invitation owners now use checked increments rather than saturation/clamping.
  Membership enforcement uses the same revision helper. Policy upserts cannot
  produce a new policy body with an unchanged revision at the storage limit.
- Invitation redemption checks use-count advancement against the configured limit.
- Added boundary unit cases and a public-port regression for atomic join failure
  on version/count exhaustion, including successful retry under the same key once
  the fixture is restored. These tests remain unexecuted; no compilation or test
  suite was run in this follow-up.

### Stored revision projection follow-up

Policy reads, management locale catalogs, application snapshots, mutation results
and policy history now validate positive stored revisions instead of projecting
invalid values as revision 1. Policy update CAS validates the current stored
revision before comparing preconditions, even when the caller supplied no expected
policy or a mismatching identifier. Corrupt storage is an invariant failure, not a
normal stale-policy conflict. The existing public unsigned revision DTOs and
valid-state CAS behavior remain unchanged. Two focused unit tests cover projection
bounds and corrupt-current versus genuinely stale preconditions; unexecuted.

### Cumulative change recheck (2026-11-10)

Re-inspected the canonical module graph, core receipt implementation, changed
mutation owners, authorization delegation, locking helpers, stored arithmetic,
GraphQL wiring and new regression fixtures against the task base. This is a
static review, not typechecking or runtime certification.

Corrections made during recheck:

- Core join of a stored `invited` membership now settles a pending application in
  the same transaction. Settlement uses a typed cause to distinguish invitation,
  direct join and leave audit provenance. Added direct-join/replay regression.
- Invitation create/revoke, governance and direct enforcement now consult admitted
  receipts before aggregate/subject lookup and locks. A replay must not depend on
  the current subject still existing. Added revoke/delete/replay regression.
- Updated localization and policy-locale source guards that still demanded the
  removed saturating arithmetic and revision clamping.

Verification obtained: all 50 `verify-groups-*.mjs` source guards were run; 38 pass,
12 fail. All 12 remaining failures also reproduce in a separate extraction of the
unchanged task base. They involve stale source/documentation/migration-count
markers or existing evidence assertions, not an executed database failure. They
are not silently waived and the overall gate is NOT green. Before guard fixes,
one previously passing base guard regressed solely because it demanded saturation;
that guard now passes. The policy-locale guard is also green again after updating
its projection marker. Tree-sitter parsed 117 module Rust files without syntax
errors before the final added regression; final syntax/whitespace results are
recorded below. Rust compilation and behavioral tests remain unexecuted by
maintainer request. Earlier selected-guard success does not describe the full
50-guard suite or subsequent intermediate edits.

Final recheck: 117 Rust files parse without syntax errors; `git diff --check`
passes. Final source-guard rerun remains 38/50, with no remaining failure that
passes on the task base. No Rust tests were run.


### Invitation and Moderation source-guard follow-up

Updated three pre-existing failing source guards to inspect the canonical owner
files instead of former inline copies or old module-layout markers:

- Invitation boundary checks hash delegation to the shared SHA-256 implementation,
  checks token removal in the transactional create/replay code, and scans both
  owner files for forbidden plaintext persistence patterns.
- Targeted delivery checks the transactional redemption/audit/admission path,
  checked member-count arithmetic, the effective public facade, and Notifications
  registration in `module.rs`. Migration registration now references the current
  nine-migration module contract, not an obsolete seven-migration inline test.
- Moderation adapter checks public export in `lib.rs` and runtime registration in
  `module.rs` separately, and explicitly rejects ignored rollback/receipt-failure
  finalization patterns.

Seven in-memory negative source probes confirmed that removing token redaction,
replacing SHA-256 delegation, restoring saturated member counts, removing factory
registrations or introducing the ignored-rollback marker makes these guards fail.
These are verifier checks only, not Rust behavior tests. No runtime readiness or
registry evidence field is promoted. The old pre-replay-lock and API-crate wording
in this plan was reconciled with the current source as part of the same follow-up.

Final guard follow-up result: 41/50 Groups source guards pass; the remaining nine
failures remain open. Rust compilation and behavioral tests were not run.


### Enforcement, history and invitation-UI source-guard follow-up

Four further guards now follow the canonical source boundary:

- Enforcement read-path migration registration is checked in `module.rs`, not
  among crate-root exports. The nine-migration contract is retained.
- Policy history checks explicit public reexports rather than requiring a glob
  export, checks checked stored revision projection, and retains append-only
  trigger, manager-authorization, CAS, editor and null-runtime-evidence checks.
- Invitation acceptance checks the actual GROUPS-05 status row for `in_progress`,
  invitation source scope and pending parity/Notifications evidence instead of
  obsolete prose sentences.
- Direct enforcement checks command admission and checked aggregate revisions;
  documentation checks follow the current shared owner mutation/provenance terms.
  The enforcement command contract now documents admission-before-replay-before-
  aggregate-lock order and the existing neutral adapter, not an unimplemented
  future API-crate split. Historical MySQL row-lock helper branches are explicitly
  distinguished from supported PostgreSQL/SQLite command admission.

Five negative in-memory source probes rejected removed migration registration,
removed public history exports, restored revision clamping, premature GROUPS-05
promotion to done, and removed enforcement admission. These are lexical verifier
checks, not executed Rust behavior tests.

Final source-guard result: 45/50 pass. Remaining failures:

- `verify-groups-localization-enforcement-expiry-postgres.mjs`
- `verify-groups-localization-graphql-postgres-parity.mjs`
- `verify-groups-localization-graphql-sqlite-parity.mjs`
- `verify-groups-membership-enforcement-graphql.mjs`
- `verify-groups-moderation-subject-postgres.mjs`

Rust compilation and behavioral tests remain deferred to the maintainer.


### Final five source-guard reconciliation

- Localization SQLite/PostgreSQL parity guards require complete aggregate-version
  equality assertions while tolerating rustfmt line wrapping and an optional
  trailing comma. They no longer require operands to occupy one source line.
- Enforcement GraphQL parity similarly requires the exact `+ 1` revoke version
  assertion on both backends. No assertion was removed or rewritten in Rust tests.
- Moderation PostgreSQL evidence permits only an explicit set of neutral root
  contract imports from `rustok-moderation`; extra service/entity/wildcard/qualified
  imports remain rejected. Persistence table-name prohibitions are retained.
- Localization expiry checks null parity/concurrency evidence directly in the FBA
  registry instead of demanding those field names in handoff prose. Missing or
  non-null evidence fields fail the guard.

Eight negative in-memory source probes rejected wrong equality expectations,
incorrect revoke version increments on both backends, a Moderation service import,
a qualified entity import and premature promotion of either localization runtime
field. These probes validate lexical guards only; they do not execute the Rust
assertions, prove behavioral parity, or establish runtime owner isolation.

Final rerun: all 50 Groups Node source guards pass. `git diff --check` passes.
No Rust source/tests were changed in this final guard slice, no Rust compilation
or tests were run, and no runtime evidence/readiness field was promoted. Earlier
38/50, 41/50 and 45/50 figures above describe historical intermediate states.

### Policy-history authorization decoupling

The history reader previously authorized by invoking the application-list port
with a one-row page, which decoded an unrelated candidate application payload.
A malformed application could therefore deny an authorized manager access to
otherwise valid policy history. The reader now validates tenant/user context and
calls the shared read-only owner-clock manager guard directly with `Moderate`.
No write locks, application queries or answer/snapshot decoding are required to
read history. Manager capability, effective suspension/ban denial, platform-manage
bypass and tenant-scoped history filtering remain owner-controlled. Invalid
actor/tenant errors use the shared context validators.

Two public-port regression tests were added: history remains readable when an
application's JSON answers violate the typed string-map contract, while application
listing itself still rejects that corruption; candidate and cross-tenant history
reads remain denied. These tests are not executed. The policy-history and boundary
source verifiers now require direct capability authorization; the history verifier
also forbids reintroducing application-list authorization. All 50 Groups source
guards pass after this change. Compilation and behavioral proof remain pending.

### Application/invitation pagination tie-breakers

Manager application lists now order by `(submitted_at DESC, id DESC)`, and manager
invitation lists by `(created_at DESC, id DESC)`. The unique identifier resolves
equal timestamps, giving each unchanged filtered dataset a total order. Existing
tenant/group/status/active-invitation filtering, page validation and public DTOs
are unchanged. This is still offset pagination: concurrent insertions, deletions,
resubmissions or time-driven invitation expiry can change page boundaries and are
not made snapshot-consistent by adding a tie-breaker.

Two unexecuted public-port regressions construct three rows with identical times,
assert exact descending UUID order across one-row pages and an empty final page,
and cover repeated application reads and both invitation activity filters. Source
guards require the unique tie-breakers, and two negative source probes reject
removing them. All 50 Groups source guards pass; no Rust tests or compilation were
run in this follow-up.

### Manager authorization before lifecycle disclosure

Invitation creation, compatibility/CAS policy upsert, application review and
manager reopen now evaluate effective manager authority under the retained owner
locks before group lifecycle/join-policy validation. Previously these commands
could disclose an archived group through `group is not active` before verifying
that the caller could manage it. Matching committed receipts still replay before
current authority or state checks. Authorized managers retain the real lifecycle
errors; candidate submission/cancellation visibility ordering is unchanged.
This closes lifecycle error-order disclosure, not every possible group-existence
side channel: aggregate location and missing-subject errors remain separate owner
lookup concerns.

An unexecuted public-port regression covers all five manager command paths against
an archived group: outsiders receive manager-required, while an authorized owner
review receives the lifecycle conflict. The effective invitation/application guard
now checks authorization-before-lifecycle within each affected function rather
than searching across adjacent implementations. All 50 source guards pass; Rust
compilation and behavioral tests remain deferred to the maintainer.

### Review membership-state precondition

Review now requires both the application and its linked stored membership to be
pending. Previously review checked pending application plus non-active/non-denied
effective membership, which allowed inconsistent left/invited membership rows to
be rewritten by approval/rejection. Approval could then reactivate a stale subject
and advance the active count from an inappropriate source state. Such mismatches
now return `groups.conflict` before any domain write; manager authorization and
enforcement denial still precede this state disclosure, and committed replay is
unchanged. Candidate activation through invitation/direct invited join retains its
separate atomic settlement path.

Added an unexecuted public-port regression covering left and invited memberships,
both approval and rejection, unchanged application/group/count on rejection, and
retry under the same key after restoring the pending fixture. The source guard
requires the pending-membership check within the review function before writing.
Rust compilation and behavioral tests remain deferred to the maintainer.

### Closed/open admission dead-end correction

Closed visibility and request admission are independent settings. Previously
`Closed + Open` returned application-required from join, while application
submission rejected the group because its policy was not Request. Core join now
admits both Public/Open and Closed/Open as active membership. Closed content stays
redacted before admission; Secret groups still require invitation authority, and
InviteOnly/Request retain their distinct admission paths. Existing invited-row
activation and suspension/ban checks remain unchanged. The owning ADR records
this contract explicitly.

Added an unexecuted public-port regression for all nine visibility/join-policy
combinations, including closed body redaction before entry, content visibility
after successful open join, success replay, exact member counts, and absence of
orphan memberships on denied/application-required entry. The join source guard
requires the corrected branch and now isolates only join, excluding the following
leave implementation from its source scan. Rust compilation and behavior tests
remain deferred to the maintainer.

### Raw application DTO validation at the transactional owner

Compatibility/CAS policy upsert, compatibility/CAS submission and focused/shared
review now invoke their existing DTO validators after context validation and
before normalization, receipt admission or owner storage access. This restores
nil-ID and raw collection/text bounds for native callers as well as GraphQL
adapters. In particular, more than 20 rule acknowledgements cannot bypass the
input bound by collapsing duplicates during normalization, and an oversized
all-whitespace review note cannot disappear before its raw bound is checked.
Valid normalized request hashing and successful receipt replay are unchanged.

An unexecuted public-port regression uses a database deliberately lacking the
owner schema, so invalid nil IDs, oversized duplicate acknowledgement lists and
oversized whitespace notes must fail with `groups.validation` without requiring
receipt or subject storage. It covers both submission modes, both upsert modes
and both review ports. The source guard checks each owner's validator precedes
normalization and transaction start. Rust compilation/tests remain deferred to
the maintainer.

### Lifecycle/invitation owner-entry validation

The same DTO-validation review found that the public effective facades invoke
transactional owner methods directly, bypassing validation in older private port
implementations. Invitation revoke and targeted accept now call their existing
validators at the transactional owner entry. Application cancel/reopen have typed
request validators and call them before idempotency lookup or transaction start.
Nil subject IDs are `groups.validation`, not receipt-table failures or
missing-subject lookups. Valid request normalization/hashing, actor validation,
receipt replay and authorization remain unchanged.

Added an unexecuted no-schema public-port regression for all four paths; the
source guard checks validator placement within each owner method before key
admission/storage. No migration or dependency changes were required. Rust tests
and compilation remain deferred to the maintainer.

### Acceptance corruption boundary follow-up

Token and targeted invitation acceptance now share the invitation owner's stored-use validator
with read projections. Negative counts and invalid limits cannot be normalized by activation.
Added an unexecuted SQLite regression for no membership/redemption/aggregate mutation on failure,
explicit storage repair followed by retry with the same key, and committed-result replay.
Compilation and PostgreSQL/SQLite runtime proof remain open; source guards are not runtime evidence.
