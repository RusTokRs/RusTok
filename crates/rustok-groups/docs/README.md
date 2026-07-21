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
- `(tenant_id, group_id, locale)` is unique;
- reads return `requested_locale`, `effective_locale`, and `available_locales`;
- writes never silently update another locale or copy fallback text into the
  requested locale;
- group handles are stable and tenant-scoped, not translation-local.

Heavy rich-text evolution may split `body` into a future `group_bodies` table.
That change must preserve one canonical body authority and must not introduce a
shadow document.

## Access and privacy

Initial visibility values:

- `public`: discoverable and publicly readable;
- `closed`: discoverable, but private content requires active membership;
- `secret`: not discoverable; every read requires membership or platform manage
  authority.

Initial join policies:

- `open`;
- `request`;
- `invite_only`.

Local roles are `owner`, `admin`, `moderator`, and `member`. They do not replace
platform RBAC. Platform RBAC authorizes operator surfaces; Groups policy authorizes
operations inside one group. Owner services re-check both boundaries.

## FBA contract

`GroupSummaryReadPort`, `GroupMembershipReadPort`, and `GroupAccessReadPort` use
`PortContext`, `PortCallPolicy`, and `PortError`.

Required context semantics:

- tenant and actor are mandatory;
- reads require a deadline;
- writes require deadline plus idempotency key;
- locale and channel are preserved across transports;
- private-content decisions fail closed when the Groups provider is unavailable;
- an unavailable optional feature provider hides or downgrades only that feature,
  never the group shell.

Consumers must not import Groups entities or query Groups tables directly.

## FFA contract

Both `admin/` and `storefront/` use:

```text
core.rs
transport.rs
transport/native_server_adapter.rs
transport/graphql_adapter.rs
ui/leptos.rs
```

- `core` has no Leptos imports;
- `transport` is the only facade used by UI;
- native `#[server]` is preferred for SSR/hydrate;
- GraphQL remains available for CSR, Next.js, mobile, and external clients;
- transport selection never falls back implicitly;
- locale comes only from host-provided `UiRouteContext.locale`;
- reusable UI primitives come from shared UI crates.

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
