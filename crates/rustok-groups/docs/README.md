# Groups module runtime contract

## Purpose

`rustok-groups` is the owner boundary for social groups inside a RusToK tenant. It combines
localized group identity, privacy, group memberships, local roles, invitations, membership
applications, feature bindings, governance, receipts, audit, and group-local enforcement.

A group membership is social participation in one group. It is not a paid subscription, billing
agreement, entitlement, organization seat, event attendance record, or chat membership.

## Responsibility zone

Groups owns:

- group identity, lifecycle, visibility, join policy, and version;
- exact-locale presentation;
- group membership lifecycle, roles, revision, and local enforcement;
- invitation creation, revocation, redemption, targeted events, and acceptance;
- application policies, exact-locale copy, history, CAS, snapshots, lifecycle, review, and bulk
  review;
- feature bindings, group-local authorization, receipts, audit, and Groups semantic events.

Groups does not own provider content, profiles, media binaries, notification delivery, search/feed
projections, moderation reports/cases/decisions, commercial subscriptions, billing, or entitlements.

## Locale contract

Language-neutral state belongs to base tables. Localized business copy belongs to exact-locale
translation rows. `PortContext.locale` is host-owned. Groups never selects English, the first stored
row, or another locale as a fallback.

Application policy identity consists of policy ID, positive revision, and exact locale. Submitted
applications preserve the exact immutable questions/rules snapshot seen by the candidate.

## Access and privacy

- Public groups expose their public shell and configured public features.
- Closed groups expose a discoverable summary; private body, membership, features, and provider
  content require effective active membership or platform authority.
- Secret groups return not-found to unauthorized users.
- Active suspension removes group-membership authority without hiding otherwise public content.
- Expired or revoked enforcement falls back immediately to stored lifecycle state.
- Legacy `status=banned` remains fail-closed for re-entry.

`GroupAccessReadPort` and `GroupMembershipEnforcementReadPort` are canonical read boundaries.
Consumers never interpret `group_memberships.status` directly for authorization.

## Transaction-aware effective authorization

Invitation and membership-application public write paths use one owner transaction for replay,
locking, effective authorization, mutation, audit, and receipt.

The synchronization order is:

```text
Group -> GroupMembership -> GroupMembershipEnforcement
```

PostgreSQL/MySQL acquire row locks. SQLite acquires writer serialization with a no-op update of the
resolved group before reading membership and enforcement state. This protects both existing and
missing membership subjects: an enforcement writer following the same protocol cannot commit
between authorization and mutation.

Command-specific locks still precede the canonical group sequence where required:

- invitation revoke/accept locks the invitation before the group;
- application review/cancel/reopen locks the application before the group;
- CAS resubmit locks an existing application before the group.

These orders do not introduce a cycle because enforcement commands do not lock invitation or
application rows.

## Replay contract

Writes require deadline and idempotency key. The owner transaction checks receipts before current
effective authorization and CAS/lifecycle re-evaluation:

- identical request hash returns the committed response;
- changed request hash conflicts;
- replay is not rejected because the actor or candidate became suspended after the original commit.

Public write facades do not perform a separate receipt/effective precheck. Read-only facades still
use the canonical read resolver.

## Invitation contract

Public services are:

- `GroupInvitationService`;
- `GroupTargetedInvitationService`.

Compatibility paths under `rustok_groups::invitations` and
`rustok_groups::targeted_invitations` remain stable, while status-only delegates are crate-private.

Manager listing requires effective active owner/admin/moderator or platform manage. Create and
revoke repeat that authorization inside the owner transaction. Token and targeted acceptance check
the exact candidate effective state inside the owner transaction and deny active suspension,
legacy banned state, and already-active membership.

Plain invitation tokens are returned only once. Persistence, audit, receipts, and events contain
only token digests and typed identifiers. Notifications remains an optional asynchronous consumer.

## Membership-application contract

Public `GroupApplicationService` implements focused read, management, CAS, lifecycle, review, and
bounded bulk-review ports while preserving `rustok_groups::applications::*` compatibility paths.

Candidate rules:

- secret groups return not-found before membership-specific denial;
- active suspension and legacy banned state deny policy/current-state reads and write actions;
- submit and CAS resubmit require an effective non-active candidate;
- cancellation requires the exact candidate and pending membership/application state.

Manager rules:

- policy writes require effective active owner/admin or platform manage;
- list/history/review/reopen require effective active owner/admin/moderator or platform manage;
- review and reopen authorize before disclosing sensitive application status;
- candidate effective state is checked in the same transaction before lifecycle mutation.

CAS rules:

- expected policy ID/revision/locale is compared under owner locks;
- stale state returns `groups.application_policy_changed`;
- no application, membership, group version, audit, or receipt mutation occurs on stale conflict;
- exact error mapping remains identical through native and GraphQL owner ports.

Bulk review:

- requires confirmation;
- accepts 1..50 unique application IDs;
- preserves request order;
- derives the existing SHA-256 child idempotency key;
- runs each item through transactional focused review;
- returns per-item success/error without rolling back successful siblings.

## FBA and transport contract

All public ports use `PortContext`, `PortCallPolicy`, and `PortError`. Reads require deadline; writes
require deadline plus idempotency key. Consumers never import Groups entities or query Groups tables.

Native and GraphQL adapters call the same public owner services. A selected transport never retries
implicitly through another path. Owner denial, timeout, and invariant failure are preserved.

## Moderation compatibility

Moderation owns reports, cases, immutable decisions, retries, appeals, and application scheduling.
Groups owns the resulting group/group-membership mutation. A future neutral adapter will call one
shared Groups enforcement command; moderation must never update Groups tables directly.

Exact membership subject identity is `group_memberships.id` plus
`group_memberships.revision`. `groups.version` and timestamps are not substitutes.

## Open gates

The following remain open:

- localization management effective authorization;
- governance role/ownership transactional effective authorization;
- provider ACL integration and remote/degraded profiles;
- leave and member-count suspension/restoration semantics;
- direct suspend/revoke command and shared owner mutation path;
- neutral moderation adapter and durable application evidence;
- PostgreSQL/SQLite migration, lock, contention, replay, parity, security, accessibility, retry, and
  recovery execution.

No Cargo check, test, migration, Node verifier, browser, or CI command was executed for this source
slice. Runtime evidence remains `null`; FFA, FBA, `GROUPS-06`, `GROUPS-07`, and `GROUPS-19` remain
`in_progress`.

## Related documents

- [Canonical implementation plan](implementation-plan.md)
- [Root module README](../README.md)
- [FBA registry](../contracts/groups-fba-registry.json)
- [Effective invitation/application contract](../contracts/groups-effective-membership-invitations-applications.json)
- [Module authoring guide](../../../docs/modules/module-authoring.md)
- [Database contract](../../../docs/architecture/database.md)
- [FFA package architecture](../../../docs/UI/module-package-architecture.md)
- [FBA architecture](../../../docs/backend/module-backend-architecture.md)
