# Groups module runtime contract

## Purpose

`rustok-groups` provides the social-container boundary for communities inside a
RusToK tenant. It combines phpFox-style modular groups with RusToK owner-module,
FFA, FBA, multilingual storage, tenant isolation, and headless transport rules.

## Responsibility Zone

### Owned state

- group identity, tenant, owner, handle, status, visibility, and join policy;
- localized title, summary, and body;
- memberships, local role and local membership state;
- feature bindings and provider-specific versioned configuration;
- group rules, membership questions, invitations, bans, audit, receipts, and
  owner events as their program slices are completed.

### State owned elsewhere

- auth users and sessions;
- profile presentation;
- media binary objects;
- forum topics and replies;
- blog posts and comments;
- Pages documents and Page Builder artifacts;
- products, marketplace listings, carts, orders, payments, and fulfillment;
- notifications inbox, search projections, feed entries, and analytics.

No Groups table has a foreign key to another optional domain module. Cross-domain
references are logical typed identifiers and are resolved through public ports.

## Multilingual database contract

The schema follows `base + translations + optional bodies`:

- `groups` contains only language-neutral identity and policy state;
- `group_translations` contains `title`, `summary`, and `body` by normalized
  `locale VARCHAR(32)`;
- `(tenant_id, group_id, locale)` is unique and the translation relation is
  tenant-composite;
- reads return `requested_locale`, `effective_locale`, and `available_locales`;
- writes never silently update another locale or copy fallback text into the
  requested locale;
- group handles are stable and tenant-scoped, not translation-local.

The host supplies the already-resolved effective locale through `PortContext`.
Groups normalizes that tag, requires the exact translation row, and never injects
an English or arbitrary first-row fallback. Tenant preference and fallback policy
remain host/runtime responsibilities; a missing effective-locale row is an
explicit unavailable/not-found presentation, not permission to select another
stored language.

Catalog and search queries are scoped to that effective locale before group
pagination. For the executable contract, catalog and search queries are scoped to
that effective locale as one selection boundary. A title match in one language
therefore cannot return a shell rendered from another language. `requested_locale`
preserves the caller evidence while `effective_locale` reports the normalized row
that was actually selected.

Localized presentation limits count Unicode scalar values rather than UTF-8 bytes,
so Cyrillic, CJK, and Latin text receive the same 240-character title and
500-character summary limits. PostgreSQL CHECK constraints and SQLite validation
triggers enforce normalized locale shape and presentation length at the DB boundary.

The base JSON objects `groups.metadata`, `group_memberships.metadata`, and
`group_feature_bindings.configuration` must remain language-agnostic. Their
reserved top-level presentation fields are `title`, `summary`, `body`, `name`,
`description`, `translations`, `localized`, `locales`, `i18n`, and `seo`. Canonical
localized copy under those fields belongs to owner translation rows or a dedicated
provider-owned localized contract. The Groups service validates public group
metadata, while PostgreSQL CHECK constraints and SQLite insert/update triggers
repeat the rule for all three objects and direct SQL writes.

This restriction is intentionally top-level. Nested provider-schema fields with
technical names such as `name` or `title` remain valid configuration when they are
not canonical localized business copy. JSON configuration is allowed; using it as
a shadow translation store is not.

### Localization management

`GroupLocalizationReadPort` lists the exact stored translation rows for one group.
`GroupLocalizationCommandPort` owns `upsert_group_translation` and
`delete_group_translation`.

- management operations require an authenticated active owner/administrator or
  platform `groups:manage` authority;
- authorization is re-checked by `GroupLocalizationService`, including inside the
  write transaction;
- locale input is normalized once and mutations target only the exact
  `(tenant_id, group_id, locale)` row;
- upsert never clones fallback copy into a requested locale;
- delete rejects removal of the final translation row;
- translation mutation and group-version increment commit in one transaction;
- current localization commands require idempotency keys but do not yet persist
  replay receipts, so replay/concurrency promotion remains blocked.

The management surface is published through typed Rust ports, the merged GraphQL
query/mutation roots, and module-owned Leptos server functions. Native and GraphQL
adapters call the same owner service. Source presence does not prove runtime result,
error, concurrency, or replay parity.

Heavy rich-text evolution may split `body` into a future `group_bodies` table.
That change must preserve one canonical body authority and must not introduce a
shadow document.

## Access and privacy

Visibility values:

- `public`: the localized shell, body, and enabled feature bindings are publicly
  readable;
- `closed`: the localized shell is discoverable, while body, feature bindings,
  member lists, and provider-owned content require active membership or platform
  manage authority;
- `secret`: not discoverable; an active member or platform manage authority is
  required even for the localized shell.

The access contract separates:

- `discover`: inclusion in catalog/search-style discovery; never true for secret
  groups without platform manage authority;
- `view_summary`: localized group shell access; closed shells are public and secret
  shells are visible only to active members or platform managers;
- `view`: private group content access; public groups allow it, while closed and
  secret groups require active membership or platform manage authority.

A denied direct shell read uses not-found semantics so secret-group existence is
not disclosed. A permitted closed shell read without `view` access returns the
localized title/summary and neutral group metadata, but redacts translation body
and feature bindings. Host-selected locale resolution never bypasses this policy.

Initial join policies:

- `open`;
- `request`;
- `invite_only`.

Local roles are `owner`, `admin`, `moderator`, and `member`. They do not replace
platform RBAC. Platform RBAC authorizes operator surfaces; Groups policy authorizes
operations inside one group. Owner services re-check both boundaries.

## FBA contract

`GroupSummaryReadPort`, `GroupMembershipReadPort`, `GroupAccessReadPort`,
`GroupLocalizationReadPort`, `GroupCommandPort`,
`GroupLocalizationCommandPort`, and `GroupGovernanceCommandPort` use
`PortContext`, `PortCallPolicy`, and `PortError`.

Required context semantics:

- tenant and actor are mandatory;
- reads require a deadline;
- writes require deadline plus idempotency key;
- locale and channel are preserved across transports;
- private-content decisions fail closed when the Groups provider is unavailable;
- an unavailable optional feature provider hides or downgrades only that feature,
  never the group shell;
- localization row and group version commit in one owner transaction;
- governance state, idempotency receipt, and immutable audit entry commit in one
  owner transaction.

Governance commands are exposed through the typed Rust port, the merged
`graphql_governance::GroupsMutationRoot`, and module-owned Leptos server functions.
All surfaces construct the same `PortContext` fields and call
`GroupGovernanceService`; they do not copy role or ownership policy. Runtime result,
error, replay, and concurrency parity remain evidence gates rather than inferred
from source presence.

Consumers must not import Groups entities or query Groups tables directly.

## FFA contract

The module-owned admin/storefront packages retain the `core → transport → UI`
shape. Localization adds dedicated native and UI files without bypassing the
facade:

```text
core.rs
transport.rs
transport/native_server_adapter.rs
transport/native_localization_adapter.rs
transport/graphql_adapter.rs
ui/leptos.rs
ui/localization.rs
ui/root.rs
```

- `core` has no Leptos imports;
- `transport` is the only facade used by UI;
- native `#[server]` is preferred for SSR/hydrate;
- GraphQL remains available for CSR, Next.js, mobile, and external clients;
- transport selection never falls back implicitly;
- locale comes only from host-provided `UiRouteContext.locale`;
- reusable UI primitives come from shared UI crates.

The admin core validates and normalizes governance UUID input and localization
UUID/locale/text input, creating a fresh idempotency key for each deliberate
mutation. The composed Leptos root renders directory, governance, and exact-locale
translation management workspaces and calls only the selected transport facade. It
does not decide local-role, ownership, or fallback policy. Group/member pickers,
confirmation, localization receipts, audit history, accessibility evidence, and
executed transport parity remain later work.

## Integration

Feature keys are namespaced and versioned. Canonical examples:

- `forum.discussions`;
- `blog.posts`;
- `pages.wiki`;
- `marketplace.store`;
- `media.gallery`;
- `events.calendar`;
- `chat.room`.

A binding expresses policy and configuration; it does not transfer persistence
ownership. The host composes the owner module's UI contribution into the group
shell. Groups must not embed another module's screens or business logic.

## Verification

The owner verification commands are documented in the implementation plan. The
minimum expected checks are:

```bash
cargo xtask module validate groups
cargo check -p rustok-groups --features graphql
cargo check -p rustok-groups-admin --features ssr
cargo check -p rustok-groups-storefront --features ssr
node scripts/verify/verify-groups-boundary.mjs
node scripts/verify/verify-db-multilingual-contract.mjs
npm run verify:i18n:ui
npm run verify:frontend:host-ffa-contract
```

## Related Documents

- [Canonical implementation plan](implementation-plan.md)
- [Root module README](../README.md)
- [FBA registry](../contracts/groups-fba-registry.json)
- [Module authoring guide](../../../docs/modules/module-authoring.md)
- [Database contract](../../../docs/architecture/database.md)
- [FFA package architecture](../../../docs/UI/module-package-architecture.md)
- [FBA architecture](../../../docs/backend/module-backend-architecture.md)
