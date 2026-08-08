---
id: doc://crates/rustok-groups/docs/implementation-plan.md
kind: module_implementation_plan
language: en
status: active
owners:
  - rustok-groups
  - platform-community
last_reviewed: 2026-08-08
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

Compatibility uses `rustok-moderation-api`; Groups never depends on moderation entities, migrations,
or owner services. Subject identity is fixed:

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

Writes require deadline and idempotency key. Established command lock ordering is preserved. A
command may lock the identity/serialization row needed to locate its owner aggregate before receipt
lookup, such as the group on invitation create or the invitation on revoke.

After those required pre-replay locks, an identical receipt is returned before current effective
authorization, CAS, lifecycle validation, or domain mutation. A changed request using the same key
conflicts. Replay is never denied because membership authority changed after the original commit.

The canonical membership lock sequence is:

```text
Group -> GroupMembership -> GroupMembershipEnforcement
```

PostgreSQL/MySQL use row locks. SQLite obtains writer serialization through a no-op update of the
already resolved group before membership/enforcement reads. Authorization runs after receipt replay
and owner locking but before the first domain mutation. Commands that lock multiple memberships use
deterministic user UUID order, followed by deterministic membership-ID enforcement-row order.

Application CAS preserves existing application-before-group ordering where an application row
already exists. Invitation/application identity locks do not create a cycle with enforcement because
the enforcement command never locks invitation or application rows. Bulk review remains one
transaction, audit, receipt, and result per item.

## Current implementation state

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
- direct GraphQL suspend/revoke mutations composed into the stable final Groups mutation root and
  routed only through `GroupMembershipEnforcementCommandPort`;
- stored lifecycle active member-count semantics that remain independent from temporary owner-clock
  suspension/expiry/revocation while every enforcement mutation still advances group version;
- append-only membership suspend/revoke semantic events beside targeted invitation events;
- effective core `GroupsService` for access, redaction, join/rejoin, membership listing, enabled
  features, and transaction-aware feature settings using the canonical effective-manager lock protocol;
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
- executed feature-settings suspension/expiry and concurrent enforcement-vs-write evidence;
- executed localization suspension/expiry, native/GraphQL parity, and concurrent enforcement-vs-write
  evidence;
- maintainer execution of the PostgreSQL and SQLite governance/enforcement evidence sources plus
  remaining governance suspension/expiry, stress/deadlock and native/GraphQL parity evidence;
- native/GraphQL parity, CAS, lifecycle, bulk-review, retry, recovery, security, and accessibility
  evidence for the broader module;
- provider ACL integration and remote/degraded profiles;
- neutral moderation adapter and durable moderation application orchestration.

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
| GROUPS-07 | in_progress | revision, enforcement read/direct command/GraphQL, effective core/feature/localization/governance access, transactional invitation/application authorization | moderation adapter, provider cutover, runtime/concurrency/parity evidence |
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
reserved for the neutral moderation adapter.

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
3. acquires any established identity/serialization row required before receipt lookup;
4. returns matching receipt replay or changed-request conflict;
5. acquires the remaining group/membership/enforcement locks;
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
- receipt-first replay after the required group serialization lock, with actor/group/request binding;
- deterministic membership/enforcement lock ordering;
- canonical reason codes and optional owner-clock expiry;
- stored lifecycle status preserved as restoration state;
- direct-local provenance with no Moderation decision identity;
- shared crate-private suspension/revocation owner mutations for the later neutral adapter;
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
boundary as enforcement/localization. Both `change_group_role` and `transfer_group_ownership` lock
the group before receipt lookup. Replay identity is bound to tenant + group + actor + command + request hash,
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

### Planned moderation adapter

Initial mapping remains:

- `GroupMembership` plus `SuspendSubject { effective_until }` maps to the shared Groups owner
  suspension mutation;
- identical decision ID/hash replays before current subject reads; changed hash conflicts;
- subject revision, tenant, group scope, effect version, hierarchy, owner invariants and immutable
  decision provenance are validated in the Groups transaction;
- the adapter uses `source_kind=moderation_decision` and stores only bounded decision UUID/hash plus
  trusted actor provenance, never case/report/queue/policy data;
- unsupported effects and account sanctions are rejected without mutation;
- moderation records applied evidence only after a matching adapter result.

The adapter is the next moderation-specific source slice and requires the neutral
`rustok-moderation-api` dependency plus producer receipt integration. It must reuse the owner mutation
above rather than introduce a second Groups enforcement state path.

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

## Remaining implementation order

1. Add the neutral moderation subject adapter over the shared enforcement owner mutation.
2. Convert provider ACL consumers and remote/degraded profiles.
3. Execute and retain the PostgreSQL/SQLite governance-enforcement evidence sources, then produce
   remaining direct-enforcement, feature-settings, localization, governance and adapter runtime,
   parity, concurrency, security, migration and accessibility evidence.

## Degraded modes

- Groups access unavailable: deny private content.
- Corrupt enforcement row: invariant failure; never infer active.
- Expired/revoked enforcement: owner-clock fallback without cleanup.
- Active suspension: remove membership authority without hiding public content or changing the
  stored lifecycle member count.
- Legacy banned membership: deny re-entry.
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
node scripts/verify/verify-groups-membership-enforcement-graphql.mjs
node scripts/verify/verify-groups-effective-membership-access.mjs
node scripts/verify/verify-groups-effective-membership-invitations-applications.mjs
npm run verify:i18n:ui
npm run verify:frontend:host-ffa-contract
```

No Cargo check, test, migration, Node verifier, browser, or CI command was executed for this source
slice. All affected runtime evidence remains `null`.