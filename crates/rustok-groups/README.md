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
- crate-root effective `GroupsService` for core access, join/rejoin, private redaction, membership
  listing, enabled features, and feature settings;
- effective invitation, targeted-invitation, and membership-application public services;
- transaction-aware invitation/application writes using one owner transaction for receipt replay,
  locking, effective authorization, mutation, audit, and receipt.

Invitation/application writes use the lock order:

```text
Group -> GroupMembership -> GroupMembershipEnforcement
```

PostgreSQL and MySQL use row locks. SQLite acquires writer serialization with a no-op group write
before reading membership/enforcement state. The effective check therefore occurs after receipt
replay and owner locking, but before the first domain mutation. Public facades no longer perform a
separate write precheck that can race the owner transaction.

The status-only implementations remain crate-private compatibility delegates. Public module paths
remain stable:

```text
rustok_groups::invitations::*
rustok_groups::targeted_invitations::*
rustok_groups::applications::*
```

Direct suspend/revoke commands, localization/governance conversion, provider ACL integration,
member-count suspension/restoration semantics, the moderation adapter/application orchestration,
and runtime evidence remain open. `GROUPS-07` remains `in_progress`.

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
- Publish `GroupMembershipEnforcementReadPort`; no public enforcement command port exists yet.

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
  A future neutral adapter will call a shared Groups enforcement command; moderation never writes
  Groups tables directly.

## Readiness

Source presence does not prove compilation, migration behavior, PostgreSQL/SQLite lock behavior,
concurrency, replay, CAS, transport parity, security, accessibility, retry, or recovery.

FFA, FBA, `GROUPS-06`, `GROUPS-07`, and `GROUPS-19` remain `in_progress`. Transaction-aware
invitation/application authorization is source-complete, but runtime evidence and the remaining
owner paths are open.

## Documentation

- [Live module contract](docs/README.md)
- [Canonical implementation plan](docs/implementation-plan.md)
- [Bulk review contract](docs/bulk-review-contract.md)
- [FBA registry](contracts/groups-fba-registry.json)
- [Effective membership access contract](contracts/groups-effective-membership-access.json)
- [Effective invitation/application contract](contracts/groups-effective-membership-invitations-applications.json)
- [Application no-bypass guard](../../scripts/verify/verify-groups-application-native-no-bypass.mjs)
- [Bulk review guard](../../scripts/verify/verify-groups-application-bulk-review.mjs)
- [Membership enforcement read guard](../../scripts/verify/verify-groups-membership-enforcement-read-path.mjs)
- [Effective membership access guard](../../scripts/verify/verify-groups-effective-membership-access.mjs)
- [Effective invitation/application guard](../../scripts/verify/verify-groups-effective-membership-invitations-applications.mjs)
