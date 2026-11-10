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
### Membership command consistency (2026-11-10)

All Groups commands admit the tenant-scoped idempotency key before reading mutable
owner state. PostgreSQL uses a transaction advisory lock derived from tenant and
key; SQLite obtains its writer reservation before any snapshot read. Receipts,
state, aggregate version, and audit commit together. This includes core commands,
not only invitation/application/governance commands. Admission precedes aggregate
and child locks; reads never acquire command admission locks.

Request-policy groups admit candidates only through application submission;
`join_group` returns `groups.application_required` rather than creating an orphan
pending membership. An approved application may be resubmitted only after its
membership has left. Invitation activation completes any pending application in
the same transaction. Leaving cancels a pending application. Application snapshots
remain stable during each attempt; resubmission replaces the current attempt as
for rejected/cancelled applications, while audit retains transition provenance.
A separate immutable multi-attempt archive is not introduced by this change.

Public membership rosters contain only stored lifecycle-active members. Managers
may read the management roster. Exact membership reads require self, authorized
access-service claims, or effective local management; secret subjects are not
revealed through an unrestricted membership lookup. Aggregate version advances
for every actual membership lifecycle mutation, independently of active count.

### Stored revision and counter bounds (2026-11-10 follow-up)

Stored aggregate/policy revisions are positive signed 64-bit values. An actual
mutation must advance its revision; neither saturation at the storage limit nor
clamping invalid values into a valid CAS token is permitted. At exhaustion or
invalid stored revision, the owner returns an invariant error and commits no
mutation, audit, or success receipt. Member-count increments similarly reject
negative counts and signed-storage overflow. Invitation redemption must advance
its use count within its configured maximum. Existing successful receipts still
replay before these checks against current state.

Revision projections follow the same invariant: invalid stored values are never
clamped into a public CAS token. Current policy corruption is an invariant error
even if the supplied precondition is absent or mismatches; ordinary policy-changed
conflicts describe only valid stored policy state.

Activation through core join of a stored invited membership also settles a pending
application atomically. Audit provenance distinguishes direct join from invitation
redemption. After admission, successful receipt replay precedes mutable subject
lookup/locking; a replay is not conditional on the subject's current existence.

### Visibility and admission policy are independent (2026-11-10)

`Closed` restricts content/roster visibility to effective members, but does not
implicitly require a membership application. Both public and closed groups with
`Open` admission allow direct join. With `Request`, both require application
submission; with `InviteOnly`, both require an invitation. Secret groups still
require invitation authority regardless of the stored admission policy. Existing
invited membership activation is preserved. This resolves the former closed/open
combination that returned application-required while rejecting applications under
its non-request policy. Joining does not bypass effective suspension/ban checks,
and closed content remains redacted before successful admission.
