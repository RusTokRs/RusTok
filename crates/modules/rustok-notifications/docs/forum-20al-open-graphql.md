# FORUM-20AL notification open GraphQL authorization

Status: source-ready / unvalidated

## Scope

`FORUM-20AL` closes the fresh notification-open authorization residual left by
`FORUM-20AK`. It adds GraphQL parity for the authenticated storefront open decision while
preserving the existing native Leptos server-function path and the owner-owned
`NotificationInboxStorefrontPort`.

This slice does not add GraphQL group-state mutations. Mark-read, mark-unread, and archive
remain native-only write operations with deadline and idempotency admission.

## Owner GraphQL boundary

The Notifications query root now exposes
`notificationInboxAuthorizeOpen(notificationId: String!)`.

The resolver:

1. requires an authenticated human-user principal;
2. rejects OAuth service principals through the same user-required envelope;
3. requires matching `AuthContext` and `TenantContext` tenant identity;
4. requires the Notifications module to be enabled;
5. validates a bounded, non-control, non-nil UUID string;
6. builds the same canonical storefront read `PortContext` used by grouped reads;
7. delegates to `NotificationInboxStorefrontPort::authorize_open`;
8. returns a typed `ALLOWED` or `UNAVAILABLE` decision.

The request cannot select tenant or recipient identity. Tenant comes from
`TenantContext`; recipient and actor come from the authenticated human user. Permissions,
effective locale, storefront channel, correlation identity, and a five-second deadline
cross the owner port.

## Fresh authorization and non-oracular behavior

The owner open service remains the sole authority. It filters the stored notification by
notification id, tenant id, and recipient id, then re-evaluates current recipient policy
and invokes the current source provider's target-open authorization.

The following cases all produce `UNAVAILABLE`:

- the notification does not exist;
- the notification belongs to another tenant;
- the notification belongs to another recipient;
- current recipient policy suppresses it;
- the source target is no longer available.

This preserves the existing non-oracular boundary. Internal database, registry, policy,
and provider failures use the stable sanitized unavailable GraphQL envelope.

A route is emitted only for `ALLOWED`, and it is the bounded internal
`NotificationTargetRoute` produced by the owner source provider. The GraphQL storefront
adapter rejects an `ALLOWED` response with no route rather than navigating with an
incomplete response.

## Selected storefront transport

The public `authorize_notification_open` function is now a compatibility wrapper over one
selected authorization facade:

- SSR and hydrate profiles select `authorize_notification_open_native`;
- CSR and headless profiles select the module-owned GraphQL query;
- no native-to-GraphQL or GraphQL-to-native fallback occurs;
- explicit transport credentials remain available through
  `authorize_notification_open_selected`;
- the UI call site and its stale interaction guards remain unchanged.

`NotificationsView` still calls fresh authorization for each open click and invokes browser
navigation only after `NotificationStorefrontOpenDecision::Allowed { route }`. An
`Unavailable` decision renders the existing no-longer-available feedback.

## Runtime composition

`FORUM-20AL` reuses the `NotificationsGraphqlRuntimeData` added by `FORUM-20AK`. Manifest
schema data composition receives the host database, materialized
`NotificationSourceRegistry`, and existing `NotificationRecipientPolicyRuntime`, then
stores only an `Arc<dyn NotificationInboxStorefrontPort>`.

No parallel registry, policy, inbox service, direct storefront SeaORM query, local storage,
or shadow inbox is introduced.

## Evidence

- owner resolver and typed decision: `crates/rustok-notifications/src/graphql.rs`;
- GraphQL storefront adapter: `crates/rustok-notifications/storefront/src/transport/graphql_adapter.rs`;
- selected transport facade: `crates/rustok-notifications/storefront/src/transport.rs`;
- unchanged guarded navigation call site: `crates/rustok-notifications/storefront/src/ui/leptos.rs`;
- source contract test: `crates/rustok-notifications/storefront/tests/open_graphql_contract.rs`;
- machine contract: `crates/rustok-forum/contracts/forum-notification-inbox-open-graphql.json`;
- static verifier: `scripts/verify/verify-forum-notification-inbox-open-graphql.mjs`.

## Pending follow-up

The following remain outside this slice:

- GraphQL group-state mutations with write admission and idempotency;
- auth-reactive automatic grouped-inbox bootstrap refresh without explicit resource refresh;
- safe synchronization of the canonical Forum plan beyond `FORUM-20G`;
- safe synchronization of the Notifications-local plan beyond `FORUM-20AA`;
- large Notifications owner README and live-contract synchronization;
- tenant-wide scheduled reconciliation and payload redaction;
- delivery enqueue and channel transports;
- delivery-time target authorization;
- PostgreSQL execution and cross-consumer runtime evidence.

Suggested maintainer validation commands are recorded in the machine contract. None were
run by the implementation agent.
