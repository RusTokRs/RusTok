# rustok-groups

## Purpose

`rustok-groups` owns social-group identity, multilingual presentation, privacy,
membership, local roles, invitations, rules, feature bindings, and group access
policy for RusToK.

A group is a social container and policy owner. It is not the persistence owner
for forum topics, blog posts, Pages documents, marketplace listings, products,
media assets, comments, notifications, or search documents.

## Responsibilities

- Own tenant-scoped group identity, handle, lifecycle, visibility, and join policy.
- Store language-neutral state in `groups` and localized title, summary, and body
  fields in `group_translations` with normalized `VARCHAR(32)` locales.
- Own memberships, local roles, membership status, invitations, bans, rules, and
  membership questions as the implementation program advances.
- Own versioned group feature bindings such as `forum.discussions`, `blog.posts`,
  `pages.wiki`, and `marketplace.store` without importing those modules' tables.
- Publish `GroupSummaryReadPort`, `GroupMembershipReadPort`, and
  `GroupAccessReadPort` for FBA consumers.
- Publish semantic group events after authoritative owner writes; optional
  consumers such as notifications, index, search, feed, and analytics must not be
  required for a group command to commit.
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
- `graphql::GroupsQuery` with the `graphql` feature
- `graphql::GroupsMutation` with the `graphql` feature
- `admin::GroupsAdmin` through the `rustok-groups-admin` package
- `storefront::GroupsView` through the `rustok-groups-storefront` package

## Interactions

- Auth/users remains the authority for credentials, sessions, and user identity.
- `rustok-profiles` supplies public member summaries; Groups never copies profile
  display state as canonical data.
- `rustok-media` owns uploads and asset lifecycle; Groups stores typed media UUID
  references only.
- Forum, Blog, Pages, Marketplace, Media Social, Events, and future modules keep
  their own persistence and consume Groups access decisions through typed ports.
- `rustok-moderation` may issue validated decisions through a Groups-owned command
  boundary; it must never update Groups tables directly.
- `rustok-index`, `rustok-search`, and notifications consume committed semantic
  events and must preserve secret/closed visibility.
- Host applications provide tenant, auth, locale, channel, route, and transport
  context only. They do not own Groups business policy or UI workflows.

## Documentation

- [Live module contract](docs/README.md)
- [Canonical implementation plan](docs/implementation-plan.md)
- [FBA registry](contracts/groups-fba-registry.json)
- [Platform documentation map](../../docs/index.md)
