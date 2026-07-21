# rustok-groups

## Purpose

`rustok-groups` owns social-group identity, multilingual presentation, privacy,
membership, local roles, invitations, feature bindings, and group access policy for
RusToK. Exact-locale translation management, bounded invitation tokens, role
delegation, ownership transfer, command receipts, immutable audit, and native/
GraphQL administration transports are implemented. Questions, rules, bans, event
publication, and full runtime evidence remain subsequent plan-led slices.

A group is a social container and policy owner. It is not the persistence owner
for forum topics, blog posts, Pages documents, marketplace listings, products,
media assets, comments, notification delivery, or search documents.

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
  audit, and receipt storage contains no plaintext token.
- Activate an accepted invitation, redemption, membership, member count, group
  version, audit entry, and command receipt in one owner transaction.
- Persist successful governance and invitation state with idempotency receipts and
  immutable audit evidence. Localization commands require idempotency keys, but
  durable localization receipts and replay evidence remain pending.
- Own versioned group feature bindings such as `forum.discussions`, `blog.posts`,
  `pages.wiki`, and `marketplace.store` without importing those modules' tables.
- Publish typed FBA ports for summary, membership, access, localization,
  invitations, commands, and governance.
- Introduce transactional semantic events only with the planned owner event/outbox
  slice. Notifications may consume those events later but never become a
  synchronous invitation dependency.
- Publish module-owned Leptos admin and storefront FFA packages with
  framework-neutral `core`, transport facade, native `#[server]`, GraphQL, and thin
  Leptos bindings.
- Publish the typed RBAC surface for `groups:*`.

## Entry points

- `GroupsModule`
- `GroupsService`
- `GroupLocalizationService`
- `GroupInvitationService`
- `GroupGovernanceService`
- `GroupSummaryReadPort`
- `GroupMembershipReadPort`
- `GroupAccessReadPort`
- `GroupLocalizationReadPort`
- `GroupInvitationReadPort`
- `GroupCommandPort`
- `GroupLocalizationCommandPort`
- `GroupInvitationCommandPort`
- `GroupGovernanceCommandPort`
- `graphql_invitations::GroupsQueryRoot` with the `graphql` feature
- `graphql_invitations::GroupsMutationRoot` with the `graphql` feature
- `rustok_groups_admin::GroupsAdmin`
- `rustok_groups_admin::load_group_admin_translations`
- `rustok_groups_admin::upsert_group_admin_translation`
- `rustok_groups_admin::delete_group_admin_translation`
- `rustok_groups_admin::load_group_admin_invitations`
- `rustok_groups_admin::create_group_admin_invitation`
- `rustok_groups_admin::revoke_group_admin_invitation`
- `rustok_groups_admin::change_group_admin_role`
- `rustok_groups_admin::transfer_group_admin_ownership`
- `rustok_groups_storefront::GroupsView`

## Interactions

- Auth/users remains the authority for credentials, sessions, and user identity.
- `rustok-profiles` supplies public member summaries; Groups never copies profile
  display state as canonical data.
- `rustok-media` owns uploads and asset lifecycle; Groups stores typed media UUID
  references only.
- Forum, Blog, Pages, Marketplace, Media Social, Events, and future modules keep
  their own persistence and consume Groups access decisions through typed ports.
- `rustok-notifications` may consume a future committed invitation event; Groups
  does not synchronously send email, push, or notification messages.
- `rustok-moderation` may issue validated decisions through a future moderation
  command adapter; it must never update Groups tables directly.
- `rustok-index`, `rustok-search`, and notifications will consume committed
  semantic events after that owner event slice exists and must preserve
  secret/closed visibility.
- Host applications provide tenant, auth, locale, channel, route, and transport
  context only. They do not own Groups business policy or UI workflows.

## Documentation

- [Live module contract](docs/README.md)
- [Canonical implementation plan](docs/implementation-plan.md)
- [FBA registry](contracts/groups-fba-registry.json)
- [Platform documentation map](../../docs/index.md)
