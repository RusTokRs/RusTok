# FORUM-20AH native notification storefront adapter

Status: **source-ready / unvalidated**

## Scope

This slice connects the transport-neutral authenticated-user inbox port from
`FORUM-20AG` to the module-owned Leptos storefront package.

The native adapter exposes server functions for:

- exact unread count;
- bounded group summaries;
- bounded exact-group item pages;
- fresh open authorization;
- bounded group mark-read, mark-unread, and archive commands.

## Trust boundary

Transport request DTOs do not accept tenant, recipient, or user identity. Every
server function extracts `AuthContext`, `TenantContext`, and `RequestContext`, rejects
an auth/tenant mismatch, and builds `PortContext` with the authenticated user as the
recipient actor.

Read calls receive a five-second deadline before owner access. The group-state write
also receives the caller-supplied idempotency key. Permissions are retained as port
claims and the channel is recorded as `storefront`.

For the open endpoint, authentication and tenant admission occur before notification
UUID parsing. Invalid IDs therefore do not bypass authentication admission.

## Runtime composition

The adapter does not construct a source registry or recipient policy. It retrieves the
materialized `Arc<NotificationSourceRegistry>` and `NotificationRecipientPolicyRuntime`
from the existing neutral `HostRuntimeContext`, then composes
`NotificationInboxStorefrontService` with the host database connection.

Missing runtime capabilities fail closed through one public unavailable message. Owner
`PortError` values remain sanitized before they become `ServerFnError` values.

The existing `rustok-storefront` `ssr` feature already enables
`rustok-notifications-storefront/ssr`, so no additional host route or application
composition file is required for the generated server-function endpoints.

## DTO mapping

The wasm-safe storefront contract uses strings for UUIDs, routes, and RFC 3339
timestamps, local snake-case enums for state, priority, and group actions, and a bounded
string map for template data. Owner paging cursors and count values pass through
without reinterpretation.

## Evidence

- adapter: `storefront/src/transport/native_server_adapter.rs`;
- public DTOs: `storefront/src/core.rs`;
- DTO boundary proof: `storefront/tests/native_transport_contract.rs`;
- machine contract:
  `rustok-forum/contracts/forum-notification-inbox-native-storefront-adapter.json`;
- static verifier:
  `scripts/verify/verify-forum-notification-inbox-native-storefront-adapter.mjs`.

## Validation status

Tests, Cargo commands, formatting commands, verifier execution, workflows, and CI were
not run by the implementation agent, per maintainer instruction.

## Remaining work

The grouped Leptos inbox view still renders its explicit unavailable state. Hydrated
paging, loading/error state, group actions, unread badge integration, GraphQL exposure,
channel delivery, scheduled reconciliation, payload redaction, and PostgreSQL runtime
execution remain separate gates.

The canonical Forum ledger, Notifications owner-local implementation ledger, and large
Notifications owner/live README files still require safe synchronization through
`FORUM-20AH`; this slice records that pending state rather than replacing those files
wholesale while unrelated work may be landing.
