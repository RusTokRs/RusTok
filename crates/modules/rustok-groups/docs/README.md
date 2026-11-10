# Groups module runtime contract

## Purpose

`rustok-groups` is the owner boundary for social groups inside a RusToK tenant. It combines
localized group identity, privacy, group memberships, local roles, invitations, membership
applications, feature bindings, governance, receipts, audit, and group-local enforcement.

A group membership is social participation in one group. It is not a paid subscription, billing
agreement, entitlement, organization seat, event attendance record, or chat membership.

## Responsibility Zone

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
- Public rosters include stored lifecycle-active members only. Pending, invited and
  banned membership states require effective management authority.
- Exact membership reads require self, effective local management or explicit
  access-service/platform claims.
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

Command admission always precedes mutable reads and aggregate/child locks.
PostgreSQL uses a transaction advisory lock for the exact tenant/idempotency key;
SQLite acquires its database writer reservation before any read snapshot. An
unlocked child lookup may locate a group, but the owner locks the group before
re-reading and locking application/invitation state. This common group-first order
also covers settlement by invitation acceptance and leave commands.

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

- secret groups return not-found before lifecycle, join-policy, policy/CAS, or membership-specific denial;
- active suspension and legacy banned state deny policy/current-state reads and write actions;
- submit and CAS resubmit require an effective non-active candidate;
- cancellation requires the exact candidate and pending membership/application state;
- an approved candidate may submit again after the previous membership has left;
- invitation activation atomically completes a pending application;
- leaving atomically cancels a pending application;
- request-policy `join_group` returns `groups.application_required` without creating membership.

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

## Integration

### FBA and transport contract

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

The 2026-11-10 remediation runs source boundary verifiers only. No Rust compilation,
Rust test, migration execution, browser or CI evidence is claimed. Runtime evidence remains `null`; FFA, FBA, `GROUPS-06`, `GROUPS-07`, and `GROUPS-19` remain
`in_progress`.

## Verification

- `cargo test -p rustok-groups`
- `cargo xtask module validate groups`

## Related documents

- [Canonical implementation plan](implementation-plan.md)
- [Root module README](../README.md)
- [Bulk review contract](./bulk-review-contract.md)
- [FBA registry](../contracts/groups-fba-registry.json)
- [Effective invitation/application contract](../contracts/groups-effective-membership-invitations-applications.json)
- [Module authoring guide](../../../../docs/modules/module-authoring.md)
- [Database contract](../../../../docs/architecture/database.md)
- [FFA package architecture](../../../../docs/UI/module-package-architecture.md)
- [FBA architecture](../../../../docs/backend/module-backend-architecture.md)

### Membership enforcement contracts

- [Membership enforcement access path integration contract](./membership-enforcement-access-path-integration-contract.md)
- [Membership enforcement command contract](./membership-enforcement-command-contract.md)
- [Membership enforcement concurrency PostgreSQL contract](./membership-enforcement-concurrency-postgres-contract.md)
- [Membership enforcement concurrency SQLite contract](./membership-enforcement-concurrency-sqlite-contract.md)
- [Membership enforcement expiry PostgreSQL contract](./membership-enforcement-expiry-postgres-contract.md)
- [Membership enforcement expiry SQLite contract](./membership-enforcement-expiry-sqlite-contract.md)
- [Membership enforcement GraphQL contract](./membership-enforcement-graphql-contract.md)
- [Membership enforcement runtime PostgreSQL contract](./membership-enforcement-runtime-postgres-contract.md)
- [Membership enforcement runtime SQLite contract](./membership-enforcement-runtime-sqlite-contract.md)
- [Membership enforcement target lock contract](./membership-enforcement-target-lock-contract.md)

### Governance enforcement contracts

- [Governance enforcement PostgreSQL contract](./governance-enforcement-postgres-contract.md)
- [Governance enforcement SQLite contract](./governance-enforcement-sqlite-contract.md)
- [Governance enforcement stress PostgreSQL contract](./governance-enforcement-stress-postgres-contract.md)
- [Governance enforcement stress SQLite contract](./governance-enforcement-stress-sqlite-contract.md)
- [Governance GraphQL PostgreSQL parity contract](./governance-graphql-postgres-parity-contract.md)
- [Governance GraphQL SQLite parity contract](./governance-graphql-sqlite-parity-contract.md)
- [Governance suspension expiry PostgreSQL contract](./governance-suspension-expiry-postgres-contract.md)
- [Governance suspension expiry SQLite contract](./governance-suspension-expiry-sqlite-contract.md)

### Localization enforcement contracts

- [Localization enforcement concurrency PostgreSQL contract](./localization-enforcement-concurrency-postgres-contract.md)
- [Localization enforcement concurrency SQLite contract](./localization-enforcement-concurrency-sqlite-contract.md)
- [Localization enforcement expiry PostgreSQL contract](./localization-enforcement-expiry-postgres-contract.md)
- [Localization enforcement expiry SQLite contract](./localization-enforcement-expiry-sqlite-contract.md)
- [Localization GraphQL PostgreSQL parity contract](./localization-graphql-postgres-parity-contract.md)
- [Localization GraphQL SQLite parity contract](./localization-graphql-sqlite-parity-contract.md)

### Feature enforcement & moderation contracts

- [Feature enforcement authorization contract](./feature-enforcement-authorization-contract.md)
- [Feature enforcement PostgreSQL contract](./feature-enforcement-postgres-contract.md)
- [Feature enforcement SQLite contract](./feature-enforcement-sqlite-contract.md)
- [Moderation subject PostgreSQL contract](./moderation-subject-postgres-contract.md)


### Subject locator and moderation finalization follow-up

SQLite command admission and exact membership-subject locators share an empty
receipt UPDATE writer reservation. Locator reads cannot open a read snapshot
before that reservation, including Moderation and public exact-subject entrypaths
that do not use the core Groups command receipt protocol.

Moderation application and receipt completion share the owner transaction.
Failure of either explicitly rolls back. A rollback failure or failure to persist
a terminal producer error returns retryable `groups.moderation_finalization_failed`
with both the original error and finalization stage in diagnostic context; neither
is silently discarded. A failed rollback is not recorded as a successfully
finalized permanent rejection.

Additional regression cases are present for secret-group privacy across active,
archived and suspended lifecycle states (including candidate cancellation of a
historical application), invalid/missing candidate targets, SQLite
reservation contention and moderation finalization error classification. They have
not been run in this follow-up; execution is reserved for the maintainer's final pass.

### Checked stored arithmetic

Actual mutations must advance a positive aggregate/policy revision. Exhaustion of
signed storage is an invariant failure, not a successful saturated write. The
same fail-closed rule applies to negative/overflowing member-count increments and
out-of-range invitation redemption counts. The transaction must not commit domain
changes, audit, or a success receipt on arithmetic failure. Existing receipts
retain their normal replay semantics. Added boundary and atomicity regressions
are awaiting the maintainer's final execution pass.

Policy/catalog/history and application-snapshot revision projections also reject
non-positive stored revisions: readers never manufacture revision 1. Policy update
CAS distinguishes corrupt current storage (invariant failure) from a valid but
stale precondition (policy-changed conflict), including absent/mismatching expected
policies. Positive `i64::MAX` remains readable but cannot be advanced by a write.

Direct join of an existing invited membership also completes any pending
application atomically, with distinct join audit provenance. Admitted receipt
replays precede current aggregate/subject lookup for invitation management,
governance and enforcement, as well as core and application commands.

Cumulative static recheck: 38 of 50 Groups source guards pass; 12 failures also
reproduce on the unchanged task base. The full gate is not green. These source
checks and syntax parsing are not compilation or runtime evidence; see the
implementation plan for the recheck scope and corrections.


Invitation/targeted-delivery and Moderation adapter source guards now inspect
canonical transactional owners and runtime registration in `module.rs`. Checks
retain token-redaction, digest, redemption and registration requirements and add
finalization anti-swallowing checks. Negative source probes validate that the
checks reject representative regressions; they do not execute Rust or establish
runtime evidence.


Enforcement read/command, policy-history and invitation-UI source guards now use
runtime module registration, explicit exports, checked revision projections and
the canonical GROUPS-05 status row. The command contract is reconciled with the
existing Moderation adapter and receipt-before-subject-lock semantics. Current
source-guard result is 45/50; the five remaining failures are listed in the plan.
No Rust runtime evidence was obtained in this follow-up.


The final parity/expiry/Moderation evidence source guards now tolerate assertion
line wrapping without dropping exact version checks, allow only enumerated neutral
Moderation imports, and check unexecuted localization evidence directly in the
registry. This completes source-guard reconciliation, not runtime certification.

Policy history authorizes directly through the shared effective-manager read
guard (`Moderate`) after tenant/user validation. It never loads candidate
applications merely to establish authority. Invalid answers/snapshots in an
unrelated application cannot break history reads; application readers still fail
closed on their own invalid payloads. Added corruption-isolation and authority/
tenant regression tests remain unexecuted.

Application management lists use `(submitted_at DESC, id DESC)`; invitation
management lists use `(created_at DESC, id DESC)`. Tied timestamps cannot leave
ordering to the database. Pages remain offset-based and do not promise a shared
snapshot across concurrent mutations or invitation expiry. Added equal-timestamp
pagination regression tests remain awaiting maintainer execution.

Manager-only invitation creation, policy upsert, review and reopen check effective
authority before group lifecycle/join-policy errors. This prevents unauthorized
callers learning archived-state details through the command error ordering.
Committed replay remains first, and candidate secret-visibility gates are
unchanged. This is not a claim of uniform missing/existing-subject responses.

Application review requires a pending application **and** a pending linked stored
membership. Left/invited mismatches fail with `groups.conflict` before mutation;
review does not repair inconsistent lifecycle state by activating or rejecting an
unrelated membership state. Authorization, enforcement and receipt-replay ordering
remain unchanged. The new regression is present but unexecuted.

Open admission permits direct entry into public **and closed** groups. Closed
visibility protects content/rosters before entry; it does not implicitly select
Request admission. Request groups use application submission, InviteOnly uses
invitations, and Secret still requires invitation authority. A stored invited
membership retains its explicit activation path. See the owning ADR for this
visibility/admission separation and the unexecuted matrix regression for coverage.

Application transactional owners validate raw DTOs before normalization and
storage: nil IDs, collection limits and raw review-note bounds apply equally to
compatibility/CAS calls and both review ports. Normalization cannot shrink an
oversized input into the accepted range. Valid requests retain normalized hashing
and receipt replay semantics. The added no-schema validation regression remains
unexecuted.

Application cancel/reopen and invitation revoke/targeted accept validate subject
IDs at their transactional owner entry, not only in a transport or private port
wrapper. Nil IDs fail as `groups.validation` before receipt/storage access. Valid
receipt replay and authorization semantics remain unchanged; the new no-schema
regression has not been executed.
