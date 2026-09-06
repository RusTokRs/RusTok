# FORUM-20AJ notification storefront navigation badge

Status: **source-ready / unvalidated**

## Scope

This slice mounts a module-owned Notifications action in the generic storefront header
without copying notification logic into the application shell.

The action provides:

- a locale-aware link to the Notifications module page;
- the exact authenticated-user unread count;
- a badge only when the exact count is greater than zero;
- best-effort failure isolation so the application header still renders when inbox access
  is unavailable.

## Owner GraphQL query

`NotificationsQuery::notification_inbox_unread_count` provides the CSR/headless parity
path required by the header surface. The resolver:

1. requires `AuthContext`;
2. rejects OAuth service principals through `is_human_user_principal()`;
3. requires matching `TenantContext`;
4. requires the Notifications module to be enabled;
5. derives `tenant_id` from the tenant context and `recipient_id` from the authenticated
   human user;
6. delegates to `NotificationInboxUnreadCountService::count_unread`.

The query accepts no tenant, recipient, or user identity argument. Database and internal
owner errors map to the generic retryable `NOTIFICATION_INBOX_UNAVAILABLE` envelope.
Validation errors retain the safe `NOTIFICATION_VALIDATION_ERROR` code. Raw database
messages are not exposed.

## Dual-path transport

`load_notification_navigation_unread_count` uses the shared
`rustok-ui-transport::execute_selected_transport` contract:

- SSR and hydrate profiles select the native server function;
- CSR/headless profiles select the GraphQL query;
- the selected path does not fall back to another transport.

Only the exact unread-count read receives GraphQL parity in this slice. Group summaries,
exact-group item pages, open authorization, and group-state commands remain native-only.

## Module-owned navigation component

`NotificationNavigation` is a no-prop Leptos component exported from the Notifications
storefront package and declared in `rustok-module.toml`.

It:

- reads locale, route segment, and query context from `UiRouteContext`;
- builds the inbox link through `module_route_base("notifications")` rather than a hardcoded
  route;
- reads the access token and tenant slug only as transport credentials;
- renders the existing `NotificationUnreadBadge` for a positive exact count;
- preserves a localized Notifications link when the exact count is zero;
- renders no visible action after authentication, module, or transport failure;
- stores no count in local storage or a package-global shadow state.

## Host composition

The host adds a generic `StorefrontSlot::HeaderActions` contract. Build-time manifest
codegen and `xtask` validate the `header_actions` string, and `StorefrontLayout` passes
ordered contributions into the header.

The header keeps `HeaderNavigation` separate from `HeaderActions`, so a module action
cannot replace the primary navigation menu. The host source contains no direct import of
`NotificationNavigation`; the module manifest remains the composition source of truth.

## Localization

The Notifications storefront package now declares English and Russian locale bundles
through `rustok-ui-i18n-leptos`. Navigation link and accessible unread-count copy resolve
from the host-provided effective locale.

## Evidence

- owner query: `src/graphql.rs`;
- dual-path facade: `storefront/src/transport.rs`;
- GraphQL adapter: `storefront/src/transport/graphql_adapter.rs`;
- module component: `storefront/src/ui/navigation.rs`;
- manifest registration: `rustok-module.toml`;
- host composition: `apps/storefront/src/app/mod.rs`,
  `apps/storefront/src/widgets/header/mod.rs`, and `apps/storefront/build.rs`;
- source evidence: `storefront/tests/navigation_badge_contract.rs`;
- host slot evidence: `apps/storefront/tests/pages_menu_layout_slots_contract.rs`;
- machine contract:
  `rustok-forum/contracts/forum-notification-navigation-badge.json`;
- static verifier:
  `scripts/verify/verify-forum-notification-navigation-badge.mjs`.

## Validation status

Tests, Cargo commands, formatting commands, verifiers, workflows, and CI were not run by
the implementation agent, per maintainer instruction.

## Remaining work

GraphQL parity for grouped inbox pages, open authorization, and group-state commands
remains a separate gate. Scheduled reconciliation, payload redaction, channel delivery,
delivery-time authorization, host trust/channel facts, non-public descriptor
materialization, search/SEO migration, and PostgreSQL runtime evidence also remain open.

The canonical Forum ledger, Notifications local plan, large Notifications owner/live
README files, and central manifest slot reference still require safe synchronization
through `FORUM-20AJ`. This slice records those pending updates instead of replacing large
concurrent documents wholesale.
