# rustok-groups

## Purpose

`rustok-groups` owns social-group identity, multilingual presentation, privacy, group
memberships, local roles, invitations, membership applications, feature bindings,
group-local enforcement state, and group access policy for RusToK.

A **group membership** is social participation in one group with a group-local role and
lifecycle. It is not a paid subscription, commercial membership plan, billing agreement, or
entitlement. Paid plans and purchased access belong to separate subscription, entitlement, and
billing owners.

A group is a social container and policy owner. It does not own provider content such as forum
topics, blog posts, Pages documents, marketplace listings, media assets, comments, notification
inboxes, search documents, moderation cases, subscriptions, billing plans, or entitlements.

## Current GROUPS-07 state

Source now exists for:

- monotonic `group_memberships.revision`;
- bounded current `group_membership_enforcements` projection;
- owner-clock effective-state resolution with expired/revoked fallback;
- direct `GroupMembershipEnforcementCommandPort` suspend/revoke with expected-revision CAS,
  receipt-first replay, local hierarchy, owner protection, group-version advance, audit/events and
  shared owner mutation functions also used by the neutral Moderation adapter;
- neutral `GroupsModerationSubjectAdapterFactory` for `groups/group_membership`, with trusted
  Moderation scope propagation, producer receipt replay, exact membership revision/group scope
  fencing and `SuspendSubject` -> Groups-owned expiry-aware enforcement;
- stored lifecycle active `groups.member_count` semantics: temporary enforcement never changes the
  counter, so owner-clock expiry cannot leave a cleanup-dependent count split;
- append-only membership suspension/revocation semantic events beside targeted invitation events;
- crate-root effective `GroupsService` for core access, join/rejoin, private redaction, membership
  listing, enabled features, and feature settings;
- effective invitation, targeted-invitation, and membership-application public services;
- transaction-aware invitation/application writes using one owner transaction for receipt replay,
  locking, effective authorization, mutation, audit, and receipt.

Invitation/application and direct enforcement writes preserve the lock family:

```text
Group -> GroupMembership -> GroupMembershipEnforcement
```

PostgreSQL and MySQL use row locks. SQLite acquires writer serialization with a no-op group write
before reading membership/enforcement state. Effective checks therefore occur after receipt replay
and owner locking, but before the first domain mutation. Direct enforcement additionally locks
actor/target memberships and enforcement rows in deterministic UUID order.

The status-only implementations remain crate-private compatibility delegates. Public module paths
remain stable:

```text
rustok_groups::invitations::*
rustok_groups::targeted_invitations::*
rustok_groups::applications::*
```

Provider ACL integration, broader native/GraphQL parity, and moderation/direct-enforcement
runtime/replay/concurrency evidence remain open. The neutral membership Moderation adapter is now
source-complete, while `GROUPS-07` remains `in_progress` until its declared runtime gates close.

## Responsibilities

### Group identity, presentation, and access

- Own tenant-scoped group identity, handle, lifecycle, visibility, join policy, member count, and
  group version.
- Store language-neutral state in `groups` and exact-locale presentation in
  `group_translations`.
- Preserve public, closed, and secret group semantics.
- Separate discoverable summary access from private body/provider access.
- Own namespaced feature bindings without importing provider persistence.

### Group memberships and governance

- Own group memberships, local roles, lifecycle state, role delegation, and ownership transfer.
- Keep owner/admin/moderator/member hierarchy in Groups rather than copying it into RBAC or
  provider modules.
- Preserve owner protection, tenant isolation, command receipts, and domain audit.
- Keep legacy `status=banned` fail-closed while migrating to expiring Groups-owned enforcement.
- Never reuse group-membership tables or ports for subscriptions, entitlements, organization
  seats, event attendance, or chat participation.

### Membership revision and enforcement

- Initialize `group_memberships.revision` at one and protect it from regression.
- Bump revision when role, lifecycle, invitation fields, or Groups-owned enforcement state changes.
- Store one bounded current enforcement row per membership.
- Never copy moderation reports, case notes, queue state, policy snapshots, or appeals into Groups.
- Evaluate expiry with the Groups UTC clock; cleanup is optional normalization, not access logic.
- Resolve effective states `missing`, `active`, `inactive`, `suspended`, and `legacy_banned`.
- Publish `GroupMembershipEnforcementReadPort` for owner-clock effective state.
- Publish `GroupMembershipEnforcementCommandPort` for direct single-membership suspend/revoke.
- Require a user actor, bounded idempotency key, exact expected membership revision, hierarchy and
  owner protection for direct enforcement.
- Keep `groups.member_count` as the stored lifecycle-active count; suspension/revocation never
  adjusts it, while every actual enforcement mutation still bumps `groups.version`.
- Allow direct revoke only for active `direct_local` enforcement. Local moderation cannot erase a
  `moderation_decision` row.
- Preserve original suspension provenance on revoke and record revoker identity in immutable
  audit/event facts.

### Effective core access

- Export one public core type: `rustok_groups::GroupsService`.
- Deny suspended members closed/secret private access and local membership authority.
- Preserve public group reads during local suspension.
- Deny join/rejoin for active suspension or legacy banned state.
- Require effective active owner/admin authority for feature settings.

### Invitations

- Own bounded invitation records, SHA-256 token digests, expiry, revocation, redemption, use count,
  membership activation, targeted invitation events, audit, and receipts.
- Require effective active owner/admin/moderator or platform manage for listing, create, and revoke.
- Deny token and targeted acceptance for active suspension or legacy banned state.
- Preserve active-member conflict and expired/revoked enforcement fallback.
- Execute receipt replay, group/membership/enforcement locking, effective candidate/manager check,
  mutation, audit, and receipt in the same owner transaction.

### Membership applications

- Own exact-locale application policies, append-only policy history, policy CAS, immutable candidate
  snapshots, lifecycle, review, and bounded bulk review.
- Preserve secret-group not-found semantics before membership-specific candidate denial.
- Require effective active owner/admin for policy writes and effective active
  owner/admin/moderator for review/reopen.
- Require effective candidate state for submit, CAS resubmit, cancel, reopen, review, and approval.
- Preserve authorization-first sensitive status disclosure.
- Preserve bulk review limits, request order, per-item transactions/results, and child idempotency
  keys while routing each item through transactional focused review.
- Preserve `groups.application_policy_changed` mapping for CAS conflicts.

### FFA/FBA composition

- Publish module-owned admin/storefront packages with framework-neutral core and explicit transport.
- Keep business logic out of host applications.
- Require providers to consume typed Groups ports instead of querying Groups tables.
- Fail closed for private content when access/enforcement evaluation is unavailable.
- Never retry through another transport implicitly.

## Entry points

Core owner/runtime:

- `GroupsModule`
- `rustok_groups::GroupsService`
- `GroupMembershipEnforcementService`
- `GroupMembershipEnforcementCommandService`
- `GroupLocalizationService`
- `GroupInvitationService`
- `GroupTargetedInvitationService`
- `GroupApplicationService`
- `GroupApplicationPolicyHistoryService`
- `GroupGovernanceService`

Primary ports:

- `GroupSummaryReadPort`
- `GroupMembershipReadPort`
- `GroupMembershipEnforcementReadPort`
- `GroupMembershipEnforcementCommandPort`
- `GroupAccessReadPort`
- `GroupLocalizationReadPort`
- `GroupInvitationReadPort`
- `GroupInvitationCommandPort`
- `GroupTargetedInvitationCommandPort`
- `GroupApplicationReadPort`
- `GroupApplicationPolicyHistoryReadPort`
- `GroupApplicationPolicyManagementReadPort`
- `GroupApplicationCasCommandPort`
- `GroupApplicationLifecycleReadPort`
- `GroupApplicationLifecycleCommandPort`
- `GroupApplicationReviewCommandPort`
- `GroupApplicationBulkReviewCommandPort`
- `GroupApplicationCommandPort` for legacy Rust compatibility only
- `GroupCommandPort`
- `GroupLocalizationCommandPort`
- `GroupGovernanceCommandPort`

## Interactions

- Auth/users owns credentials, sessions, and user identity.
- Profiles owns canonical profile presentation.
- Media owns uploads and asset lifecycle.
- Forum, Blog, Pages, Marketplace, Events, Chat, and future providers own their persistence and
  consume Groups access ports.
- Notifications may consume committed targeted-invitation events asynchronously.
- Moderation owns reports, cases, decisions, retries, appeals, and application orchestration.
  The neutral adapter calls the shared Groups enforcement owner mutation after producer receipt,
  exact trusted scope, subject revision and effect validation; moderation never writes Groups tables
  directly.

## Readiness

Source presence does not prove compilation, migration behavior, PostgreSQL/SQLite lock behavior,
concurrency, replay, CAS, transport parity, security, accessibility, retry, or recovery.

FFA, FBA, `GROUPS-06`, `GROUPS-07`, and `GROUPS-19` remain `in_progress`. Transaction-aware
invitation/application authorization, direct enforcement, and the neutral membership Moderation
adapter are source-complete, but runtime evidence and remaining provider/owner paths are open.

## Documentation

- [Live module contract](docs/README.md)
- [Canonical implementation plan](docs/implementation-plan.md)
- [Membership enforcement command contract](docs/membership-enforcement-command-contract.md)
- [Bulk review contract](docs/bulk-review-contract.md)
- [FBA registry](contracts/groups-fba-registry.json)
- [Effective membership access contract](contracts/groups-effective-membership-access.json)
- [Effective invitation/application contract](contracts/groups-effective-membership-invitations-applications.json)
- [Application no-bypass guard](../../scripts/verify/verify-groups-application-native-no-bypass.mjs)
- [Bulk review guard](../../scripts/verify/verify-groups-application-bulk-review.mjs)
- [Membership enforcement read guard](../../scripts/verify/verify-groups-membership-enforcement-read-path.mjs)
- [Membership enforcement command guard](../../scripts/verify/verify-groups-membership-enforcement-command.mjs)
- [Moderation membership adapter guard](../../scripts/verify/verify-groups-moderation-subject-adapter.mjs)
- [Effective membership access guard](../../scripts/verify/verify-groups-effective-membership-access.mjs)
- [Effective invitation/application guard](../../scripts/verify/verify-groups-effective-membership-invitations-applications.mjs)
