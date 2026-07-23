---
id: doc://crates/rustok-groups/docs/implementation-plan.md
kind: module_implementation_plan
language: en
status: active
owners:
  - rustok-groups
  - platform-community
last_reviewed: 2026-07-23
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

### Commands, replay, and locking

Writes require deadline and idempotency key. Established command lock ordering is preserved. A
command may lock the identity/serialization row needed to locate its owner aggregate before receipt
lookup, such as the group on invitation create or the invitation on revoke.

After those required pre-replay locks, an identical receipt is returned before current effective
authorization, CAS, lifecycle validation, or domain mutation. A changed request using the same key
conflicts. Replay is never denied because membership authority changed after the original commit.

For invitation/application effective authorization, the canonical membership lock sequence is:

```text
Group -> GroupMembership -> GroupMembershipEnforcement
```

PostgreSQL/MySQL use row locks. SQLite obtains writer serialization through a no-op update of the
already resolved group before membership/enforcement reads. Authorization runs after receipt replay
and owner locking but before the first domain mutation.

Application CAS preserves existing application-before-group ordering where an application row
already exists. Invitation/application identity locks do not create a cycle with the future
enforcement command because that command will not lock invitation or application rows. Bulk review
remains one transaction, audit, receipt, and result per item.

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
- effective core `GroupsService` for access, redaction, join/rejoin, membership listing, enabled
  features, and feature settings;
- sealed effective public invitation/application services with compatibility module paths;
- transaction-aware invitation/application writes using the group/membership/enforcement lock
  protocol;
- secret application non-disclosure, authorization-first status handling, receipt-first replay,
  stable effective error codes, CAS conflict mapping, and per-item bulk semantics.

Evidence still open:

- compilation and executed unit/integration tests;
- PostgreSQL and SQLite migration/runtime evidence;
- lock behavior and concurrent enforcement-change tests;
- native/GraphQL parity, replay, CAS, lifecycle, bulk-review, retry, recovery, security, and
  accessibility evidence;
- localization and governance effective authorization;
- provider ACL integration and remote/degraded profiles;
- leave/member-count suspension/restoration semantics;
- direct suspend/revoke command, shared owner mutation path, moderation adapter, and durable
  moderation application orchestration.

## Program ledger

| ID | Status | Scope | Remaining gate |
|---|---|---|---|
| GROUPS-00 | in_progress | ADR, ownership map, phpFox parity, FFA/FBA contracts | executable architecture review |
| GROUPS-01 | in_progress | module skeleton, manifest, RBAC, migrations, host composition | build/module validation |
| GROUPS-02 | in_progress | identity, localization, visibility, join policy, features, audit/events | lifecycle/runtime/concurrency |
| GROUPS-03 | in_progress | memberships, join/leave, roles, ownership transfer | remaining enforcement integration |
| GROUPS-04 | in_progress | typed summary/membership/access/localization/invitation/application/governance ports | consumer/fallback runtime matrix |
| GROUPS-05 | in_progress | GraphQL/native transports, invitation acceptance/delivery | parity and Notifications evidence |
| GROUPS-06 | in_progress | localized policy, CAS, lifecycle, focused/bulk review, FFA UX | profiles/events/parity/concurrency/accessibility |
| GROUPS-07 | in_progress | revision, enforcement read model, effective core access, transactional invitation/application authorization | remaining owner paths, direct command, adapter, runtime/concurrency evidence |
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
- No Groups dependency on moderation owner persistence exists.

The database trigger bridge remains transitional. The final write architecture requires an explicit
shared Groups enforcement command used by direct actions and the neutral moderation adapter.

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

### Planned owner enforcement command

The next command slice adds `GroupMembershipEnforcementCommandPort` for one direct suspend/revoke
operation with:

- expected membership revision CAS;
- owner/admin/moderator hierarchy and owner protection;
- receipt-first replay and changed-hash conflict;
- the same group/membership/enforcement lock order;
- explicit restoration lifecycle state;
- atomic membership revision, enforcement row, member count/group version, audit, event, and receipt;
- a shared owner method callable by the later moderation adapter.

No bulk enforcement command is introduced before single-command runtime evidence.

### Planned moderation adapter

Initial mapping remains:

- `GroupMembership` plus `SuspendSubject { effective_until }` maps to the shared Groups command;
- identical decision ID/hash replays; changed hash conflicts;
- subject revision, tenant, scope, effect version, hierarchy, and owner invariants are validated in
  the Groups transaction;
- unsupported effects and account sanctions are rejected without mutation;
- moderation records applied evidence only after a matching adapter result.

### GROUPS-07 definition of done

- no ownership leakage between Groups and moderation;
- monotonic membership identity/revision evidence;
- permanent, expiring, revoked, and restored enforcement across every owner path;
- hierarchy, owner protection, tenant isolation, replay, stale revision, member count, and
  concurrency evidence;
- native/GraphQL parity for state and direct actions;
- missing/timeout/retry/lost-response adapter behavior;
- moderation-disabled mode preserves existing Groups enforcement;
- PostgreSQL/SQLite migration, lock, compatibility, accessibility, and no-fallback evidence.

## Remaining implementation order

1. Convert localization management authorization to the transaction-aware resolver.
2. Convert governance role/ownership commands with owner protection and the same lock protocol.
3. Define leave and member-count suspension/restoration semantics.
4. Add one direct suspend/revoke command and shared owner mutation path.
5. Add the neutral moderation subject adapter.
6. Convert provider ACL consumers and remote/degraded profiles.
7. Produce runtime, parity, concurrency, security, and accessibility evidence.

## Degraded modes

- Groups access unavailable: deny private content.
- Corrupt enforcement row: invariant failure; never infer active.
- Expired/revoked enforcement: owner-clock fallback without cleanup.
- Active suspension: remove membership authority without hiding public content.
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
node scripts/verify/verify-groups-boundary.mjs
node scripts/verify/verify-groups-invitations-boundary.mjs
node scripts/verify/verify-groups-membership-applications.mjs
node scripts/verify/verify-groups-application-policy-cas.mjs
node scripts/verify/verify-groups-application-lifecycle.mjs
node scripts/verify/verify-groups-application-bulk-review.mjs
node scripts/verify/verify-groups-membership-enforcement-read-path.mjs
node scripts/verify/verify-groups-effective-membership-access.mjs
node scripts/verify/verify-groups-effective-membership-invitations-applications.mjs
npm run verify:i18n:ui
npm run verify:frontend:host-ffa-contract
```

No Cargo check, test, migration, Node verifier, browser, or CI command was executed for this source
slice. All affected runtime evidence remains `null`.
