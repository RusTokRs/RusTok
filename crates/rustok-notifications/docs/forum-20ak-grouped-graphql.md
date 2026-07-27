# FORUM-20AK grouped notification inbox GraphQL reads

Status: **source-ready / unvalidated**

## Scope

This slice adds GraphQL parity for the authenticated storefront inbox read plane:

- exact unread count remains available through GraphQL;
- bounded grouped-summary paging is now available through GraphQL;
- bounded exact-group item paging is now available through GraphQL;
- existing storefront UI read calls route through the selected native/GraphQL facade.

Fresh target-open authorization and group-state commands remain native-only. This slice
contains no GraphQL mutation and does not expand write authority.

## Runtime composition

`rustok-module.toml` declares `graphql::attach_schema_data` as the Notifications GraphQL
runtime-data factory. Manifest-generated server composition supplies neutral
`GraphqlRuntimeInputs`; the Notifications factory reads:

- the host database connection;
- the already materialized `Arc<NotificationSourceRegistry>`;
- the already composed `NotificationRecipientPolicyRuntime`.

When both owner runtime values are present, the factory builds the existing
`NotificationInboxStorefrontPort` through
`in_process_notification_inbox_storefront_port`. It does not create a parallel registry,
recipient policy, inbox repository, or host-owned Notifications adapter. If either owner
runtime value is unavailable, grouped reads fail through the same generic capability
unavailable envelope rather than weakening policy.

## GraphQL admission and scope

`notificationInboxGroupSummaries` and `notificationInboxGroupItems` share the same
admission helper. Before module or owner access, the resolver:

1. requires `AuthContext`;
2. rejects OAuth service principals;
3. requires `TenantContext`;
4. rejects authentication/tenant mismatch.

The resolver then requires the Notifications module to be enabled. Tenant and recipient
identity never appear in GraphQL arguments. They are derived from authenticated context
and encoded into a `PortContext` with:

- the canonical `AuthContext::port_actor()` user actor;
- effective request locale, with tenant default fallback;
- authenticated permission claims;
- channel `storefront`;
- a unique correlation id;
- a five-second read deadline.

The owner port revalidates user-only scope and deadline semantics before delegating to the
existing group-summary and group-item services.

## Read semantics

Grouped summaries and group items preserve the owner contracts already used by the native
adapter:

- limits are converted from GraphQL `Int`, reject negative/overflow values, and are
  owner-clamped to the existing maximum;
- cursors remain opaque, versioned, bounded owner cursors;
- exact group keys remain owner-validated and bounded;
- each returned latest item or item is rechecked through current recipient policy and
  source target authorization;
- suppressed, missing, and foreign-recipient results remain non-oracular;
- empty pages may still carry continuation when raw rows were suppressed;
- reads mutate no inbox state and enqueue no delivery attempt.

## Wire contract

The GraphQL DTO does not expose persistence models or arbitrary JSON:

- notification and actor UUIDs are strings;
- timestamps are RFC 3339 strings;
- item state and priority are GraphQL enums;
- bounded template data is emitted as an ordered list of `{ key, value }` fields;
- summary/item pages expose only owner cursor, `hasMore`, counts, and authorized items.

The storefront GraphQL adapter maps those DTOs into the same serializable storefront
models used by native server functions.

## Transport selection

`NotificationStorefrontTransportContext` carries only access token and tenant slug as HTTP
transport credentials. The selected read facade uses:

- native server functions for SSR and hydrate profiles;
- GraphQL for CSR and headless profiles;
- no automatic cross-path fallback.

The existing `load_notification_unread_count`,
`load_notification_group_summaries`, and `load_notification_group_items` functions remain
source-compatible UI wrappers. They resolve current transport credentials and delegate to
explicit-context selected functions. Native raw read functions remain exported with
`_native` suffixes for integration evidence.

## Error boundary

Owner `PortError` values are mapped deliberately:

- safe validation and forbidden envelopes preserve their stable code/message;
- unavailable, timeout, invariant, not-found, and conflict outcomes collapse to the generic
  `NOTIFICATION_INBOX_UNAVAILABLE` public envelope;
- database, provider, and recipient-policy details are not exposed.

## Evidence

- owner GraphQL/runtime data: `src/graphql.rs`;
- manifest runtime factory: `rustok-module.toml`;
- selected read facade: `storefront/src/transport.rs`;
- GraphQL client adapter: `storefront/src/transport/graphql_adapter.rs`;
- source contract: `storefront/tests/grouped_graphql_contract.rs`;
- machine contract:
  `rustok-forum/contracts/forum-notification-inbox-grouped-graphql.json`;
- static verifier:
  `scripts/verify/verify-forum-notification-inbox-grouped-graphql.mjs`.

## Validation status

Tests, Cargo commands, formatting commands, verifiers, workflows, and CI were not run by
the implementation agent, per maintainer instruction.

## Remaining work

GraphQL fresh-open authorization and GraphQL group-state commands remain separate
security/write gates. Scheduled reconciliation, payload redaction, channel delivery,
delivery-time authorization, host trust/channel facts, non-public descriptor
materialization, search/SEO migration, and PostgreSQL runtime evidence also remain open.

The canonical Forum ledger, Notifications local plan, and large Notifications owner/live
README files still require safe synchronization through `FORUM-20AK`. This slice records
those updates as pending instead of replacing large concurrent documents wholesale.
