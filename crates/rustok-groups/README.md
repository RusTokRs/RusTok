# rustok-groups

## Purpose

`rustok-groups` owns social-group identity, multilingual presentation, privacy,
membership, local roles, invitations, membership applications, feature bindings,
and group access policy for RusToK. Exact-locale translation management, bounded
invitation tokens, targeted invitation source events, localized application policies,
append-only policy revision history, explicit policy-locale management, atomic policy
preconditions, candidate application lifecycle, application review, role delegation,
ownership transfer, command receipts, immutable audit, and native/GraphQL transports
are implemented at source level.

Bans, bulk review, complete legacy application-command API removal, consumer-side
notification fan-out, and full runtime evidence remain plan-led work.

A group is a social container and policy owner. It is not the persistence owner for
forum topics, blog posts, Pages documents, marketplace listings, products, media
assets, comments, notification inbox/delivery, or search documents.

## Responsibilities

- Own tenant-scoped group identity, handle, lifecycle, visibility, and join policy.
- Store language-neutral state in `groups` and localized title, summary, and body in
  `group_translations` with normalized `VARCHAR(32)` locales.
- Consume the host-resolved effective locale without English or arbitrary first-row
  fallback.
- Publish exact-locale translation management, preserving last-row rejection and
  atomic group-version updates.
- Separate discoverable shell access (`view_summary`) from private content access
  (`view`), with closed redaction and secret-group non-disclosure.
- Own memberships, local roles, membership state, role delegation, and atomic
  ownership transfer.
- Own bounded invitation records, SHA-256 token digests, expiry, revocation, use
  counts, redemptions, and targeted invitation source events.
- Own one current membership-application policy per group and exact-locale ordered
  questions/rules.
- Store one tenant/group/user application with policy identity, revision, locale,
  immutable policy snapshot, answers, acknowledgements, status, and review metadata.
- Revalidate required answers and rule acknowledgements in the owner service. Only
  active `request` groups accept applications, and secret groups use not-found
  semantics.
- Capture every successful policy translation INSERT/UPDATE in
  `group_membership_policy_revisions` in the same database transaction. Revision rows
  reject UPDATE and DELETE.
- Publish manager-only policy history through typed Rust, GraphQL, native server
  function, and admin FFA surfaces.
- Publish `GroupApplicationPolicyManagementReadPort` for:
  - an authorized catalog of existing policy locales;
  - an explicit selected-locale management view;
  - an empty view without precondition when no policy exists;
  - an empty view with current policy ID/revision when the selected translation is
    missing, enabling CAS-safe creation.
- Keep `PortContext.locale` host-owned. The selected management locale is carried in a
  normalized typed request and never substituted into request/UI locale context.
- Publish `GroupApplicationCasCommandPort` for interactive policy saves and candidate
  submissions. Commands carry rendered policy ID, revision, and exact locale.
- Lock an existing candidate application before the group during CAS resubmit; for a
  first submission, repeat the application lookup after the group lock before writes.
- Compare expected policy before changing policy, membership, application, group
  version, audit, or receipt state. A mismatch returns
  `groups.application_policy_changed`.
- Check an identical idempotent receipt before re-evaluating the policy precondition,
  so a committed command can replay after later policy revisions.
- Publish `GroupApplicationLifecycleReadPort` for the authenticated candidate's exact
  tenant/group application without cross-candidate enumeration.
- Publish `GroupApplicationLifecycleCommandPort` for pending-only candidate
  cancellation and rejected/cancelled-only manager reopen.
- Publish `GroupApplicationReviewCommandPort` as the focused review write boundary used
  by final GraphQL and module-owned native admin surfaces.
- Candidate cancellation moves membership to `left`, marks the application
  `cancelled`, and preserves the submitted snapshot.
- Manager reopen restores membership/application to `pending`, clears prior review
  metadata, and preserves policy identity, snapshot, answers, acknowledgements, and
  submitted time.
- Keep fresh rejected/cancelled resubmit separate from reopen: it uses current-policy
  CAS and replaces the snapshot only after a successful new submission.
- Publish a visual policy editor with an existing/new exact-locale datalist. Locale
  changes invalidate loaded CAS state; save remains disabled until the selected
  management view is loaded.
- Require explicit selected-locale reload after stale policy conflict.
- Publish storefront lifecycle/stale-form recovery that preserves
  `apply=<group_uuid>` after cancellation or errors, blocks invalid duplicate submit,
  clears old answers on explicit stale reload, and clears route only after successful
  fresh submission.
- Review applications through owner/admin/moderator authorization. Approval activates
  membership and increments member count; rejection moves membership to `left`.
- Persist successful governance, invitation, and membership-application commands with
  idempotency receipts and immutable audit evidence. Localization replay receipts
  remain pending.
- Append `groups.invitation.targeted_created` without token data and register a neutral
  `NotificationSourceProvider` factory resolving at most one exact recipient.
- Own namespaced feature bindings such as `forum.discussions`, `blog.posts`,
  `pages.wiki`, and `marketplace.store` without importing provider tables.
- Keep Notifications optional. Groups owner commands do not synchronously depend on
  inbox, preference, fan-out, retry, email, or push persistence.
- Publish module-owned Leptos admin/storefront FFA packages with framework-neutral
  core, transport facade, native `#[server]`, GraphQL, and thin UI bindings.
- Publish the typed RBAC surface for `groups:*`.

The older unconditional policy-save and candidate-submit methods on
`GroupApplicationCommandPort` remain available only for Rust source compatibility.
Final GraphQL, module-owned admin/storefront FFA, and routable native policy writes do
not use them. Application review uses the focused `GroupApplicationReviewCommandPort`.
Complete removal or versioned deprecation of the legacy Rust methods remains a separate
API migration gate.

## Entry points

- `GroupsModule`
- `GroupsService`
- `GroupLocalizationService`
- `GroupInvitationService`
- `GroupTargetedInvitationService`
- `GroupApplicationService`
- `GroupApplicationPolicyHistoryService`
- `GroupGovernanceService`
- `GroupSummaryReadPort`
- `GroupMembershipReadPort`
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
- `GroupCommandPort`
- `GroupLocalizationCommandPort`
- `GroupInvitationCommandPort`
- `GroupTargetedInvitationCommandPort`
- `GroupApplicationCommandPort` for legacy Rust compatibility only
- `GroupGovernanceCommandPort`
- `graphql_application_cas::GroupsQueryRoot` with policy history, policy management,
  lifecycle, and core query composition
- `graphql_application_cas::GroupsMutationRoot` with core, localization, governance,
  invitations, CAS, focused review, and lifecycle composition
- `rustok_groups_admin::GroupsAdmin`
- `rustok_groups_admin::load_group_admin_application_policy_locale_catalog`
- `rustok_groups_admin::load_group_admin_application_policy_for_management`
- `rustok_groups_admin::load_group_admin_application_policy` compatibility facade
- `rustok_groups_admin::upsert_group_admin_application_policy`
- `rustok_groups_admin::load_group_admin_application_policy_revisions`
- `rustok_groups_admin::load_group_admin_membership_applications`
- `rustok_groups_admin::review_group_admin_membership_application`
- `rustok_groups_admin::reopen_group_admin_membership_application`
- `rustok_groups_admin::load_group_admin_translations`
- `rustok_groups_admin::upsert_group_admin_translation`
- `rustok_groups_admin::delete_group_admin_translation`
- `rustok_groups_admin::load_group_admin_invitations`
- `rustok_groups_admin::create_group_admin_invitation`
- `rustok_groups_admin::revoke_group_admin_invitation`
- `rustok_groups_admin::change_group_admin_role`
- `rustok_groups_admin::transfer_group_admin_ownership`
- `rustok_groups_storefront::GroupsView`
- `rustok_groups_storefront::load_groups_storefront_application_policy`
- `rustok_groups_storefront::load_groups_storefront_my_application`
- `rustok_groups_storefront::submit_groups_storefront_membership_application`
- `rustok_groups_storefront::cancel_groups_storefront_membership_application`
- `rustok_groups_storefront::accept_groups_storefront_targeted_invitation`

## Interactions

- Auth/users remains the authority for credentials, sessions, and user identity.
- `rustok-profiles` supplies public member summaries; Groups never copies canonical
  profile display state.
- `rustok-media` owns uploads and asset lifecycle; Groups stores typed media UUID
  references only.
- Forum, Blog, Pages, Marketplace, Media Social, Events, and future modules retain
  their own persistence and consume Groups access through typed ports.
- `rustok-notifications-api` supplies the neutral source-provider contract. Groups
  registers a deferred factory without depending on Notifications persistence.
- `rustok-notifications` may consume committed targeted-invitation events. Groups does
  not synchronously send email, push, or notification messages.
- `rustok-moderation` may use a future validated command adapter; it must never update
  Groups tables directly.
- `rustok-index` and `rustok-search` may consume committed semantic events in later
  slices while preserving closed/secret visibility.
- Host applications provide tenant, auth, effective locale, channel, route, and
  transport context. Management selection remains an explicit Groups request field;
  hosts do not own Groups business policy or UI workflows.

## Readiness

Source presence does not prove migration, runtime, locale-catalog parity, replay,
concurrency, lock ordering, security, accessibility, retry, or recovery behavior.
FFA, FBA, GROUPS-06, and GROUPS-19 remain `in_progress`; policy-revision,
application-policy-CAS, application-lifecycle, and policy-locale-management runtime
evidence keys remain `null`.

## Documentation

- [Live module contract](docs/README.md)
- [Canonical implementation plan](docs/implementation-plan.md)
- [FBA registry](contracts/groups-fba-registry.json)
- [Application no-bypass static guard](../../scripts/verify/verify-groups-application-native-no-bypass.mjs)
- [Platform documentation map](../../docs/index.md)
