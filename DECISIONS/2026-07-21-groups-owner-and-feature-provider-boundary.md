# ADR: Groups owner and feature-provider boundary

- Status: Accepted
- Date: 2026-07-21
- Owners: `rustok-groups`, platform community, platform architecture

## Context

RusToK needs phpFox-class social groups that behave as modular micro-social
networks. A group may expose discussions, posts, wiki/pages, media, events,
marketplace/store, chat, and future third-party sections. Implementing those
sections inside one Groups crate would transfer ownership, create circular
persistence dependencies, and make independent FFA/FBA deployment impossible.

The platform also requires tenant isolation, multilingual storage, explicit
transport selection, headless parity, and fail-closed privacy for closed and
secret groups.

## Decision

`rustok-groups` is the authoritative owner of:

- group identity, stable tenant-scoped handle, status, visibility, and join
  policy;
- localized group presentation;
- membership, local roles, invitations, questions, rules, bans, ownership
  transfer, and local moderation state;
- versioned namespaced feature bindings;
- access decisions and semantic group events.

It is not the owner of content produced by another domain. Forum topics, Blog
posts, Pages documents, marketplace listings, media assets, events, messages,
comments, notifications, feed entries, and search documents remain with their
owner modules.

### Backend boundary

Groups publishes typed summary, membership, access, and command ports using
`PortContext`, `PortCallPolicy`, and `PortError`. Consumers never import Groups
entities or query Groups tables. Owner content modules re-check group access on
their own authoritative commands and reads.

Private content fails closed when the Groups access provider is unavailable.
Feature-provider failure degrades only that section to `hidden`, `readonly`, or
`unavailable`; it never disables the group shell and never falls back to direct
SQL.

### Feature identity

A feature is identified by a namespaced key and contract version, for example:

- `forum.discussions`;
- `blog.posts`;
- `pages.wiki`;
- `marketplace.store`;
- `media.gallery`.

The binding stores policy and provider-owned versioned configuration. It does not
copy provider state or UI. Hosts compose owner-owned UI entrypoints.

### Multilingual storage

Language-neutral state lives in `groups`. Localized title, summary, and body live
in `group_translations` with normalized `VARCHAR(32)` locale and a unique
`(tenant_id, group_id, locale)` key. Reads expose requested/effective/available
locale evidence. A fallback read never creates or updates a translation.

### Frontend boundary

Groups admin and storefront packages use the FFA `core -> transport -> ui`
decomposition. Native `#[server]` and GraphQL remain explicit parallel paths.
Locale is supplied by the host through `UiRouteContext.locale`. Groups UI does
not embed another module's workflow.

## Consequences

- New group sections require a provider contract rather than a Groups schema
  expansion.
- Forum, Blog, Pages, Marketplace, Search, Notifications, Moderation, and future
  social modules can evolve or deploy independently.
- Secret-group non-disclosure is enforced consistently at owner boundaries.
- A dedicated Wall/Feed/Reactions implementation must be introduced by its own
  owner module; Groups must not create substitute wall/feed tables.
- FFA/FBA status remains `in_progress` until executable parity, provider-order,
  fallback, retry, and recovery evidence exists.

## Rejected alternatives

### Hard-coded feature bitmask

Rejected as the canonical registry because it cannot safely identify arbitrary
third-party providers, versions, schemas, health, or degraded modes. A computed
bitset may be introduced later only as a cache/projection.

### Foreign keys from every content module to Groups

Rejected because optional modules must remain independently owned and because
cross-module migration ordering and remote extraction would become coupled.
Logical typed context references and ports are used instead.

### Host-owned group composition

Rejected because host applications are composition roots, not domain or UI
owners. Manifest-driven contributions preserve modularity and independent
transports.