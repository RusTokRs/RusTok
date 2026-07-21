# rustok-groups

## Purpose

`rustok-groups` owns social-group identity, multilingual presentation, privacy,
membership, local roles, invitations, membership applications, feature bindings,
and group access policy for RusToK. Exact-locale translation management, bounded
invitation tokens, targeted invitation source events, localized application policies,
application review, role delegation, ownership transfer, command receipts, immutable
audit, and native/GraphQL transports are implemented at source level. Bans, bulk
review, policy revision history, consumer-side notification fan-out, and full runtime
evidence remain subsequent plan-led slices.

A group is a social container and policy owner. It is not the persistence owner
for forum topics, blog posts, Pages documents, marketplace listings, products,
media assets, comments, notification inbox/delivery, or search documents.

## Responsibilities

- Own tenant-scoped group identity, handle, lifecycle, visibility, and join policy.
- Store language-neutral state in `groups` and localized title, summary, and body
  fields in `group_translations` with normalized `VARCHAR(32)` locales.
- Consume the host-resolved effective locale for public reads without an English or
  first-row fallback.
- Publish owner-managed exact-locale list/upsert/delete operations. Translation
  mutation and group-version increment are atomic, write paths serialize on the
  group row where row locking is supported, and the last translation cannot be
  deleted.
- Separate discoverable shell access (`view_summary`) from private content access
  (`view`): closed shells are visible with body/feature redaction, while secret
  shells remain undisclosed to non-members.
- Own memberships, local roles, membership status, role delegation, and atomic
  ownership transfer.
- Own invitation records, token digests, expiry, revocation, bounded use counts, and
  unique redemptions. Targeted invitations are single-use; shareable links are
  limited to 100 uses and 30 days.
- Return invitation plaintext only from the first create response. Invitation,
  audit, receipt, and semantic-event storage contains no plaintext token.
- Activate an accepted invitation, redemption, membership, member count, group
  version, audit entry, and command receipt in one owner transaction.
- Persist successful governance, invitation, and membership-application commands
  with idempotency receipts and immutable audit evidence. Localization commands
  require idempotency keys, but durable localization receipts and replay evidence
  remain pending.
- Own one current membership-application policy per group and exact-locale policy
  translations containing bounded questions and rules. Locale fallback remains a
  host/runtime responsibility.
- Store one tenant/group/user application with the exact policy revision, locale,
  questions, rules, answers, and acknowledged rule keys seen at submission time.
- Revalidate required answers and acknowledgements in the owner service. Secret
  groups return not-found semantics and only `request` join-policy groups accept
  applications.
- Review applications through owner/admin/moderator authorization. Approval activates
  membership and increments member count; rejection moves membership to `left`.
  Application, membership, group version, audit, and receipt commit together.
- Append `groups.invitation.targeted_created` to the owner-owned, append-only
  `group_domain_events` table in the same database transaction as targeted invite
  creation. The event contains invitation/group/recipient identifiers only.
- Register a neutral `NotificationSourceProvider` factory for targeted invitations.
  It resolves at most one exact recipient and authorizes an internal
  `/modules/groups?invitation=<uuid>` route only while the invitation and group remain
  active.
- Accept targeted invitations by authenticated invitation ID through
  `GroupTargetedInvitationCommandPort`; wrong-recipient and unavailable state use
  not-found semantics. Shareable invitations continue to require the opaque token.
- Own versioned group feature bindings such as `forum.discussions`, `blog.posts`,
  `pages.wiki`, and `marketplace.store` without importing those modules' tables.
- Publish typed FBA ports for summary, membership, access, localization,
  invitations, membership applications, targeted invitation acceptance, commands,
  and governance.
- Keep Notifications optional. Invitation creation commits even when Notifications
  is not compiled or tenant-enabled; inbox, preferences, fan-out, retry, and delivery
  remain owned by `rustok-notifications`.
- Publish module-owned Leptos admin and storefront FFA packages with
  framework-neutral `core`, transport facade, native `#[server]`, GraphQL, and thin
  Leptos bindings.
- Publish the typed RBAC surface for `groups:*`.

## Entry points

- `GroupsModule`
- `GroupsService`
- `GroupLocalizationService`
- `GroupInvitationService`
- `GroupTargetedInvitationService`
- `GroupApplicationService`
- `GroupGovernanceService`
- `GroupSummaryReadPort`
- `GroupMembershipReadPort`
- `GroupAccessReadPort`
- `GroupLocalizationReadPort`
- `GroupInvitationReadPort`
- `GroupApplicationReadPort`
- `GroupCommandPort`
- `GroupLocalizationCommandPort`
- `GroupInvitationCommandPort`
- `GroupTargetedInvitationCommandPort`
- `GroupApplicationCommandPort`
- `GroupGovernanceCommandPort`
- `graphql_applications::GroupsQueryRoot` with the `graphql` feature
- `graphql_applications::GroupsMutationRoot` with application policy, submission,
  review, invitation, localization, governance, and core mutations
- `rustok_groups_admin::GroupsAdmin`
- `rustok_groups_admin::load_group_admin_application_policy`
- `rustok_groups_admin::upsert_group_admin_application_policy`
- `rustok_groups_admin::load_group_admin_membership_applications`
- `rustok_groups_admin::review_group_admin_membership_application`
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
- `rustok_groups_storefront::submit_groups_storefront_membership_application`
- `rustok_groups_storefront::accept_groups_storefront_targeted_invitation`

## Interactions

- Auth/users remains the authority for credentials, sessions, and user identity.
- `rustok-profiles` supplies public member summaries; Groups never copies profile
  display state as canonical data.
- `rustok-media` owns uploads and asset lifecycle; Groups stores typed media UUID
  references only.
- Forum, Blog, Pages, Marketplace, Media Social, Events, and future modules keep
  their own persistence and consume Groups access decisions through typed ports.
- `rustok-notifications-api` supplies the neutral source-provider contract. Groups
  registers a deferred factory without depending on Notifications persistence.
- `rustok-notifications` may materialize the Groups source and consume committed
  targeted-invitation events. Groups does not synchronously send email, push, or
  notification messages.
- `rustok-moderation` may issue validated decisions through a future moderation
  command adapter; it must never update Groups tables directly.
- `rustok-index` and `rustok-search` will consume committed semantic events in later
  slices and must preserve secret/closed visibility.
- Host applications provide tenant, auth, locale, channel, route, and transport
  context only. They do not own Groups business policy or UI workflows.

## Documentation

- [Live module contract](docs/README.md)
- [Canonical implementation plan](docs/implementation-plan.md)
- [FBA registry](contracts/groups-fba-registry.json)
- [Platform documentation map](../../docs/index.md)
