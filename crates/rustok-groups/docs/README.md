# Groups module runtime contract

## Purpose

`rustok-groups` provides the social-container boundary for communities inside a
RusToK tenant. It combines phpFox-style modular groups with RusToK owner-module,
FFA, FBA, multilingual storage, tenant isolation, and headless transport rules.

A group membership is social participation in one group. It is not a paid
subscription, commercial membership plan, billing agreement, or entitlement.

## Responsibility zone

Groups owns group identity, localized presentation, visibility, join policy,
group memberships, local roles, invitations, membership applications, ordered
questions and rules, policy locale management, policy revision history,
application lifecycle, feature bindings, group-local enforcement state, command
receipts, audit, and Groups semantic source events.

Groups does not own auth users/sessions, profile presentation, media binaries,
Forum, Blog, Pages, Marketplace, comments, notification inbox/delivery, search
projections, feed entries, checkout, payment, orders, fulfillment, subscriptions,
billing plans, entitlements, moderation cases, or analytics.

No Groups table has a foreign key to another optional domain module. Cross-domain
references are logical typed identifiers resolved through public ports.

## Multilingual database and locale contract

Language-neutral state belongs to base tables. Localized business copy belongs to
parallel exact-locale rows:

- `groups` stores language-neutral identity and policy state;
- `group_translations` stores title, summary, and body;
- `group_membership_policies` stores current language-neutral application-policy
  revision/enabled state;
- `group_membership_policy_translations` stores ordered questions and rules by
  normalized `locale VARCHAR(32)`;
- `group_membership_policy_revisions` stores append-only exact-locale snapshots of
  successful policy writes;
- `group_membership_applications` stores exact policy ID, revision, locale, and
  immutable question/rule snapshot seen by the candidate.

Candidate presentation uses the host-resolved effective locale in `PortContext`.
Groups normalizes it and selects only that row. It never injects an English
fallback, arbitrary first row, or another stored locale.

Policy management is deliberately separate:

- `PortContext.locale` remains the host request/UI locale;
- selected policy locale is a normalized field on a typed owner request;
- locale catalog returns only existing translation locales in ascending order;
- management read selects only the requested locale;
- missing policy returns an empty view without policy ID/revision;
- missing translation on an existing policy returns an empty view with current
  policy ID/revision and `translation_exists=false`;
- native and GraphQL adapters never substitute selected policy locale into request
  locale context or locale headers;
- missing copy is explicit empty/unavailable state, never fallback.

Application `policy_snapshot` and policy revision rows are immutable evidence, not
shadow localization stores. Current canonical policy copy remains the exact-locale
translation row.

## Effective membership and privacy

`group_memberships.status` is stored lifecycle compatibility state. Effective
membership authority comes from the Groups-owned owner-clock resolver combining the
membership row with `group_membership_enforcements`.

- `public`: localized shell and public features remain readable;
- `closed`: summary shell is discoverable, while body, members, features, and
  provider content require effective active membership or platform authority;
- `secret`: unauthorized users receive not-found semantics;
- active suspension is not active membership for private content, post/comment,
  invitation, application, management, ownership transfer, or provider ACLs;
- legacy `status=banned` remains fail-closed for re-entry;
- expired or revoked enforcement falls back to stored lifecycle immediately and
  does not depend on a cleanup worker.

The crate-root `rustok_groups::GroupsService` is the only public core service. Its
implementation and status-only delegate are crate-private. Module-owned GraphQL and
native surfaces therefore cannot explicitly bypass effective core access.

## Invitation contract

`GroupInvitationReadPort`, `GroupInvitationCommandPort`, and
`GroupTargetedInvitationCommandPort` own manager listing, create/revoke/token
acceptance, and authenticated targeted accept-by-ID.

Public compatibility paths remain:

- `rustok_groups::invitations::*`;
- `rustok_groups::targeted_invitations::*`;
- crate-root `GroupInvitationService` and `GroupTargetedInvitationService`.

Those service names resolve only to effective facades. The original status-only
services are crate-private delegates.

Effective facade rules:

- list/create/revoke requires effective active owner/admin/moderator or platform
  `groups:manage`;
- a suspended local manager has no invitation-management authority;
- token and targeted acceptance reject active suspension and legacy banned state;
- already-active membership remains a conflict;
- expired/revoked enforcement falls back to stored lifecycle;
- an existing command receipt delegates before effective precheck so matching replay
  or changed-request conflict remains owner-controlled.

Targeted invitations are single-use. Shareable links permit at most 100 uses and
expire within 300 seconds to 30 days. Plaintext is returned only by the first create
response; persistence, audit, receipts, and semantic events contain no recoverable
plaintext. Redemption, membership activation, member count, group version, audit,
and receipt commit in one legacy owner transaction.

Targeted insert appends `groups.invitation.targeted_created` to append-only
`group_domain_events` through a database trigger. The event carries only typed
invitation/group/recipient/actor identifiers. Notifications inbox, preferences,
fan-out, email/push, retry, and cleanup remain Notifications-owner responsibilities.

The effective invitation check currently occurs in the public facade before the
legacy owner transaction. Same-transaction effective recheck and enforcement-change
race evidence remain open.

## Membership-application contract

Public compatibility paths remain under `rustok_groups::applications::*`, but
`GroupApplicationService` resolves only to the effective facade. The status-only
application service is crate-private.

### Candidate policy and lifecycle reads

`GroupApplicationReadPort::read_group_application_policy` exposes current policy for
the host-resolved exact locale. One current language-neutral policy exists per group,
each locale contains at most 20 questions and 20 rules, and there is no module-local
fallback.

`GroupApplicationLifecycleReadPort::read_my_group_membership_application` returns
only the authenticated actor's current application. Effective suspension and legacy
banned state deny candidate policy/current-state reads through the public facade.

### Policy management and history

`GroupApplicationPolicyManagementReadPort` exposes locale catalog and selected-locale
management views. Both require effective active owner/admin or platform manage.
Candidates cannot enumerate the catalog or views.

Every successful application-policy translation INSERT/UPDATE is captured into
`group_membership_policy_revisions` in the same database transaction. History rows
are append-only. `GroupApplicationPolicyHistoryReadPort` inherits the effective
application-list manager boundary and therefore requires effective active
owner/admin/moderator or platform manage.

### Atomic policy preconditions

Interactive policy save and candidate submit use `GroupApplicationCasCommandPort`:

- `upsert_group_application_policy_if_current`;
- `submit_group_membership_application_if_current`.

Both requests carry `GroupApplicationPolicyPrecondition` with policy ID, positive
revision, and exact locale. Existing owner behavior remains:

1. validate deadline, idempotency, tenant, actor, locale, and bounds;
2. return an identical committed receipt before policy precondition re-evaluation;
3. lock the declared application/group rows where supported;
4. repeat stored-state authorization and group checks;
5. compare current policy ID, revision, and locale;
6. return `groups.application_policy_changed` before mutation on mismatch;
7. commit policy/application state, group version, audit, and receipt atomically.

The effective facade adds owner-clock manager/candidate prechecks while preserving
receipt-first replay. Same-transaction effective authorization is still an open
convergence gate.

The older unconditional save and submit methods on `GroupApplicationCommandPort`
remain available for Rust compatibility. Final GraphQL and module-owned FFA do not
expose them, and they now pass through the effective public facade.

### Submission, cancellation, reopen, and review

Only active non-secret `request` groups accept applications. Effective suspension,
legacy banned state, and active membership reject new legacy or CAS submission.
Pending or approved applications cannot be resubmitted; rejected/cancelled
applications may receive a fresh current-policy snapshot.

Candidate cancellation:

- accepts only the exact candidate;
- requires effective candidate state that is not suspended or legacy banned;
- accepts only pending application/membership state;
- moves membership to `left` and application to `cancelled`;
- preserves submitted policy identity, locale, snapshot, answers, and acknowledgements;
- retains receipt-first replay.

Manager reopen:

- requires effective active owner/admin/moderator or platform manage;
- requires target candidate not suspended, legacy banned, or already active;
- accepts rejected/cancelled application with `left` membership;
- restores membership/application to `pending` while preserving the snapshot.

Single review:

- requires effective active owner/admin/moderator or platform manage;
- requires target candidate not suspended, legacy banned, or already active;
- accepts only pending applications;
- approve activates membership and increments member count;
- reject moves membership to `left`;
- review note is optional and bounded to 2,000 characters.

### Bounded bulk review

`GroupApplicationBulkReviewCommandPort` retains:

- explicit confirmation;
- 1..50 unique application IDs;
- global note validation before item work;
- one child idempotency key derived from SHA-256 of base key plus application UUID;
- request-order results;
- one owner transaction/audit/receipt per item;
- partial success/failure result.

The effective facade implements the same loop through effective single review per
item, so suspended managers or candidates produce item errors without collapsing the
whole batch.

## FBA contract

Published ports include summary, membership, enforcement read, access, localization,
invitation, targeted invitation, application read, policy history/management, CAS,
lifecycle, focused/bulk review, group command, and governance boundaries. All use
`PortContext`, `PortCallPolicy`, and `PortError`. Reads require deadline; writes require
deadline plus idempotency key. Consumers never import Groups entities or query Groups
tables directly.

Final GraphQL composition retains core, localization, governance, invitation,
targeted invitation, application, and history fields. GraphQL and module-owned native
adapters construct crate-root service names, which now resolve to effective facades.
There is no implicit transport fallback.

## FFA contract

Admin and storefront packages retain `core → transport → UI` separation. UI imports
only transport facades, never raw adapters. Selected native or GraphQL transport never
falls back implicitly.

The admin policy editor keeps exact-locale CAS behavior. The application workspace
filters pending/approved/rejected/cancelled rows, supports focused/bulk review and
manager reopen, and receives effective manager denials through the selected transport.

Storefront uses `apply=<group_uuid>` to read current candidate status and load
host-resolved exact-locale policy. Effective suspension or legacy banned state disables
candidate application reads/actions rather than falling back to stored `active` or
`pending` status.

## Degraded modes

- Groups provider unavailable: deny private content.
- Effective resolver unavailable or corrupt: deny sensitive invitation/application
  operations; never fall back to status-only authority in the public facade.
- Existing receipt: delegate before current effective precheck so owner replay/conflict
  semantics remain authoritative.
- Effective facade precheck passes but enforcement changes before legacy transaction:
  behavior has no completed concurrency evidence; do not claim same-transaction auth.
- Candidate exact-locale policy unavailable: disable form; never choose another locale.
- Management locale catalog unavailable: disable locale selection/save.
- Policy CAS conflict: write no owner state and require explicit selected-locale reload.
- Lifecycle command transport failure: preserve selected-path error; never retry through
  another transport.
- Policy history unavailable: hide history rather than synthesizing revisions.
- Profiles unavailable: display stable UUID/placeholder, never copy canonical profile
  state.
- Notifications unavailable: owner commands commit and remain authoritative.
- Search/index unavailable: owner writes commit; projections may catch up later.

## Open gates

The following remain source or evidence work:

- same-transaction effective recheck in invitation/application owner writes;
- enforcement-change concurrency/race evidence;
- localization and governance effective authorization;
- provider ACL and remote adapter integration;
- leave/member-count suspension and restoration semantics;
- direct suspend/revoke owner command and moderation adapter;
- remove or version-deprecate legacy unconditional application methods;
- ProfilesReader summaries and application semantic events;
- locale translation deletion/lifecycle policy if required;
- migration, native/GraphQL parity, replay, stale/locale/lifecycle races, lock ordering,
  accessibility, security, retry, and recovery evidence.

## Verification

Expected commands before readiness promotion:

```bash
cargo xtask module validate groups
cargo check -p rustok-groups --features graphql
cargo check -p rustok-groups-admin --features ssr
cargo check -p rustok-groups-storefront --features ssr
cargo test -p rustok-groups
node scripts/verify/verify-groups-boundary.mjs
node scripts/verify/verify-groups-localization-boundary.mjs
node scripts/verify/verify-groups-invitations-boundary.mjs
node scripts/verify/verify-groups-targeted-invitation-delivery.mjs
node scripts/verify/verify-groups-membership-applications.mjs
node scripts/verify/verify-groups-membership-policy-revisions.mjs
node scripts/verify/verify-groups-application-policy-cas.mjs
node scripts/verify/verify-groups-application-lifecycle.mjs
node scripts/verify/verify-groups-application-policy-locales.mjs
node scripts/verify/verify-groups-application-bulk-review.mjs
node scripts/verify/verify-groups-membership-enforcement-read-path.mjs
node scripts/verify/verify-groups-effective-membership-access.mjs
node scripts/verify/verify-groups-effective-membership-invitations-applications.mjs
node scripts/verify/verify-db-multilingual-contract.mjs
npm run verify:i18n:ui
npm run verify:frontend:host-ffa-contract
```

No build, test, migration, verifier, parity, replay, concurrency, accessibility,
security, retry, or recovery command was executed for this source slice. FFA, FBA,
GROUPS-06, GROUPS-07, and GROUPS-19 remain `in_progress`.

## Related documents

- [Canonical implementation plan](implementation-plan.md)
- [Root module README](../README.md)
- [FBA registry](../contracts/groups-fba-registry.json)
- [Effective invitation/application contract](../contracts/groups-effective-membership-invitations-applications.json)
- [Module authoring guide](../../../docs/modules/module-authoring.md)
- [Database contract](../../../docs/architecture/database.md)
- [FFA package architecture](../../../docs/UI/module-package-architecture.md)
- [FBA architecture](../../../docs/backend/module-backend-architecture.md)
