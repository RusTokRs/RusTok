# rustok-groups

## Purpose

`rustok-groups` owns social-group identity, multilingual presentation, privacy,
membership, local roles, feature bindings, and group access policy for RusToK.
Invitations, questions, rules, bans, audit, and event publication are tracked as
subsequent owner slices in the canonical implementation plan.

A group is a social container and policy owner. It is not the persistence owner
for forum topics, blog posts, Pages documents, marketplace listings, products,
media assets, comments, notifications, or search documents.

## Responsibilities

- Own tenant-scoped group identity, handle, lifecycle, visibility, and join policy.
- Store language-neutral state in `groups` and localized title, summary, and body
  fields in `group_translations` with normalized `VARCHAR(32)` locales.
- Own memberships, local roles, and membership status in the current slice;
  invitations, bans, rules, and membership questions remain plan-led work.
- Own versioned group feature bindings such as `forum.discussions`, `blog.posts`,
  `pages.wiki`, and `marketplace.store` without importing those modules' tables.
- Publish `GroupSummaryReadPort`, `GroupMembershipReadPort`,
  `GroupAccessReadPort`, and `GroupCommandPort` for FBA consumers.
- Introduce transactional semantic events only with the planned owner
  event/outbox slice; optional consumers must never become a synchronous command
  dependency.
- Publish module-owned Leptos admin and storefront FFA packages with
  framework-neutral `core`, transport facade, native `#[server]`, GraphQL, and a
  thin Leptos adapter.
- Publish the typed RBAC surface for `groups:*`.

## Entry points

- `GroupsModule`
- `GroupsService`
- `GroupSummaryReadPort`
- `GroupMembershipReadPort`
- `GroupAccessReadPort`
- `GroupCommandPort`
- `graphql::GroupsQuery` with the `graphql` feature
- `graphql::GroupsMutation` with the `graphql` feature
- `rustok_groups_admin::GroupsAdmin`
- `rustok_groups_storefront::GroupsView`

## Interactions

- Auth/users remains the authority for credentials, sessions, and user identity.
- `rustok-profiles` supplies public member summaries; Groups never copies profile
  display state as canonical data.
- `rustok-media` owns uploads and asset lifecycle; Groups stores typed media UUID
  references only.
- Forum, Blog, Pages, Marketplace, Media Social, Events, and future modules keep
  their own persistence and consume Groups access decisions through typed ports.
- `rustok-moderation` may issue validated decisions through a future Groups-owned
  command boundary; it must never update Groups tables directly.
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
