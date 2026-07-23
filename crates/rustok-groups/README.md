# rustok-groups

## Purpose

`rustok-groups` owns social-group identity, multilingual presentation, privacy,
memberships, local roles, invitations, membership applications, feature bindings,
group-local enforcement state, and group access policy for RusToK.

Exact-locale translation management, bounded invitation tokens, targeted invitation source
events, localized application policies, append-only policy history, policy CAS, candidate
lifecycle, authorization-first review, bounded partial-result bulk review, role delegation,
ownership transfer, command receipts, immutable audit, and native/GraphQL transports exist at
source level.

The current GROUPS-07 foundation additionally provides a monotonic membership revision,
bounded current enforcement projection, owner-clock effective-state resolver, and
`GroupMembershipEnforcementReadPort`. Direct suspend/revoke commands, full access-path
integration, moderation adapter/application orchestration, and runtime evidence remain open.

A group is a social container and policy owner. It is not the persistence owner for forum
topics, blog posts, Pages documents, marketplace listings, products, media assets, comments,
notification inbox/delivery, search documents, or moderation cases/decisions.

## Responsibilities

### Group identity, presentation, and access

- Own tenant-scoped group identity, handle, lifecycle, visibility, join policy, member count,
  and group version.
- Store language-neutral state in `groups` and exact-locale title, summary, and body in
  `group_translations`.
- Consume the host-resolved locale without English or arbitrary first-row fallback.
- Separate discoverable shell access (`view_summary`) from private content access (`view`).
- Preserve closed-group redaction and secret-group non-disclosure.
- Own namespaced feature bindings such as `forum.discussions`, `blog.posts`, `pages.wiki`,
  and `marketplace.store` without importing provider tables or UI business trees.

### Memberships and governance

- Own memberships, local roles, lifecycle state, role delegation, and atomic ownership
  transfer.
- Keep owner/admin/moderator/member hierarchy in Groups rather than copying RBAC state into
  provider modules.
- Preserve owner protection and tenant-scoped command/audit/receipt identity.
- Keep legacy `status=banned` fail-closed for re-entry while migrating to expiring owner
  enforcement state.

### Membership revision and enforcement read foundation

- Add `group_memberships.revision`, initialized at one and protected from regression.
- Bump membership revision when role, lifecycle status, invitation lifecycle fields, or
  Groups-owned enforcement state changes.
- Use database revision guards as a compatibility bridge while legacy command owners migrate
  to one explicit shared owner command path.
- Store one bounded current `group_membership_enforcements` row per membership with tenant,
  group, user, state, reason, source, effective interval, restoration state, actor, optional
  moderation decision provenance, revision, revocation, and timestamps.
- Never copy reports, cases, policy snapshots, queue state, appeals, or arbitrary moderation
  JSON into Groups enforcement persistence.
- Publish `GroupMembershipEnforcementReadPort` and `GroupMembershipEnforcementService` for
  exact-user or authorized Groups access reads.
- Evaluate current state using the Groups UTC clock. Future, expired, or revoked enforcement
  falls back to stored membership lifecycle without requiring a cleanup worker.
- Return effective states `missing`, `active`, `inactive`, `suspended`, or `legacy_banned`,
  plus membership revision, bounded provenance, and fail-closed access booleans.
- Treat the new projection as read-only in this slice: no public enforcement command port and
  no moderation adapter is published yet.
- Keep status-only access-path conversion explicitly open for core access, invitations,
  applications, governance, provider ACLs, and member-count transitions.

### Invitations and membership applications

- Own bounded invitation records, SHA-256 token digests, expiry, revocation, redemption,
  use counts, membership activation, and targeted invitation source events.
- Keep Notifications optional; Groups commands do not synchronously depend on inbox,
  preference, email, push, retry, or fan-out persistence.
- Own one current membership-application policy per group, exact-locale ordered questions and
  rules, append-only policy revision history, and one current application per
  tenant/group/user.
- Preserve exact policy identity, locale, immutable question/rule snapshot, answers,
  acknowledgements, status, and review metadata on application rows.
- Publish focused policy-management, CAS, lifecycle, review, and bounded bulk-review ports.
- Replay identical receipts before CAS precondition re-evaluation.
- Authorize managers before sensitive application status disclosure.
- Use one transaction/audit/receipt per bulk-review item and never silently fall back between
  native and GraphQL transports.

### FFA/FBA and module composition

- Publish module-owned Leptos admin/storefront packages with framework-neutral core,
  explicit transport facade, native server functions, GraphQL adapters, and thin UI.
- Publish typed RBAC permissions for `groups:*`.
- Keep Groups business logic out of host applications.
- Keep provider modules authoritative for their persistence and consume Groups access via
  typed ports.
- Fail closed for private content when Groups access/enforcement evaluation is unavailable.

## Entry points

Core owner/runtime:

- `GroupsModule`
- `GroupsService`
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
- `GroupApplicationReadPort`
- `GroupApplicationPolicyHistoryReadPort`
- `GroupApplicationPolicyManagementReadPort`
- `GroupApplicationLifecycleReadPort`
- `GroupApplicationCasCommandPort`
- `GroupApplicationLifecycleCommandPort`
- `GroupApplicationReviewCommandPort`
- `GroupApplicationBulkReviewCommandPort`
- `GroupCommandPort`
- `GroupLocalizationCommandPort`
- `GroupInvitationCommandPort`
- `GroupTargetedInvitationCommandPort`
- `GroupApplicationCommandPort` for legacy Rust compatibility only
- `GroupGovernanceCommandPort`

No `GroupMembershipEnforcementCommandPort` is published in the current read-only slice.

## Interactions

- Auth/users remains authoritative for credentials, sessions, and user identity.
- `rustok-profiles` supplies member summaries; Groups never copies canonical profile display
  state.
- `rustok-media` owns uploads and asset lifecycle; Groups stores typed media references only.
- Forum, Blog, Pages, Marketplace, Media Social, Events, and future modules retain their own
  persistence and consume Groups access through typed ports.
- `rustok-notifications-api` supplies the neutral source-provider contract; Notifications may
  consume committed targeted-invitation events asynchronously.
- `rustok-moderation-api` supplies neutral subject/effect/adapter contracts. A later Groups
  adapter will call the shared Groups enforcement owner command; `rustok-moderation` must
  never update Groups tables directly.
- Index/search/feed consumers may consume committed semantic events while preserving
  closed/secret visibility.
- Host applications provide tenant, auth, locale, channel, route, transport, and runtime
  composition only.

## Readiness

Source presence does not prove compilation, migrations, revision-trigger behavior,
owner-clock expiry handling, complete access integration, replay, concurrency, security,
transport parity, accessibility, retry, or recovery.

FFA, FBA, GROUPS-06, GROUPS-07, and GROUPS-19 remain `in_progress`. The current enforcement
slice is a migration and read contract only; direct product suspension and moderation-driven
application are not claimed as complete.

## Documentation

- [Live module contract](docs/README.md)
- [Canonical implementation plan](docs/implementation-plan.md)
- [Bulk review contract](docs/bulk-review-contract.md)
- [FBA registry](contracts/groups-fba-registry.json)
- [Application no-bypass guard](../../scripts/verify/verify-groups-application-native-no-bypass.mjs)
- [Bulk review guard](../../scripts/verify/verify-groups-application-bulk-review.mjs)
- [Membership enforcement read guard](../../scripts/verify/verify-groups-membership-enforcement-read-path.mjs)
